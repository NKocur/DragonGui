use serde::Deserialize;
use serde_json::{Map, Value};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::css_style::{StylesheetOrigin, StylesheetStore};
use crate::style::{parse_grid_template_tracks_value, ColorRef, GridTrackSize, NodeStyle};
use crate::theme::{parse_web_color, Theme};

// ---------------------------------------------------------------------------
// Top-level typed document used only for window geometry.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct StartupDoc {
    pub window: WindowDoc,
}

#[derive(Debug, Deserialize)]
pub struct WindowDoc {
    pub props: WindowProps,
}

#[derive(Debug, Deserialize)]
pub struct WindowProps {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

// ---------------------------------------------------------------------------
// Scatter payload format
// ---------------------------------------------------------------------------

/// Wire format for scatter point data embedded in `NodeProps` or sent live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScatterPayloadFormat {
    /// Packed little-endian float32 xyz triples, 12 bytes per point.
    #[default]
    XyzF32V0,
    /// Packed little-endian PointInstance records, 32 bytes per point:
    /// x, y, z, size, r, g, b, alpha (all f32).
    PointInstanceV1,
}

impl ScatterPayloadFormat {
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "point_instance_v1" => Self::PointInstanceV1,
            _ => Self::XyzF32V0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::XyzF32V0 => "xyz_f32_v0",
            Self::PointInstanceV1 => "point_instance_v1",
        }
    }
}

// ---------------------------------------------------------------------------
// Line plot payload format
// ---------------------------------------------------------------------------

/// Wire format for 2D line plot data embedded in `NodeProps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinePlotPayloadFormat {
    /// Packed little-endian float32 xy pairs, 8 bytes per point.
    #[default]
    XyF32V0,
}

impl LinePlotPayloadFormat {
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            _ => Self::XyF32V0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinePlotSeriesProp {
    pub label: Option<String>,
    pub color: Option<ColorRef>,
    pub line_style: String,
    pub points: Vec<[f32; 2]>,
    pub bounds: Option<[f32; 4]>,
    pub payload_format: LinePlotPayloadFormat,
    pub declared_point_count: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct LinePlotHoverProp {
    pub screen: [f32; 2],
    pub plot: [f32; 2],
    pub label: Option<String>,
    pub color: Option<ColorRef>,
}

#[derive(Debug, Clone, Default)]
pub struct HistogramProp {
    pub edges: Vec<f32>,
    pub counts: Vec<f32>,
    pub color: Option<ColorRef>,
    pub label: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub mode: String,
    pub cumulative: bool,
    pub show_grid: bool,
    pub show_axes: bool,
    pub show_ticks: bool,
    pub show_toolbar: bool,
    pub tick_count: usize,
    pub auto_fit: bool,
    pub x_min: Option<f32>,
    pub x_max: Option<f32>,
    pub y_min: Option<f32>,
    pub y_max: Option<f32>,
    pub interaction: String,
    pub selection_rect: Option<[f32; 4]>,
    pub bar_gap: f32,
}

// ---------------------------------------------------------------------------
// Python theme parsing
// ---------------------------------------------------------------------------

/// Parse a `Theme` from the top-level `"theme"` key in the app document.
///
/// Returns `None` if no `"theme"` key is present; callers fall back to
/// `Theme::dark()`.  Unknown or malformed color values are replaced with
/// the corresponding dark-theme default.
pub fn parse_theme_from_doc(doc: &serde_json::Value) -> Option<Theme> {
    let t = doc.get("theme")?;
    let dark = Theme::dark();
    Some(Theme {
        background: parse_theme_color(t.get("background")).unwrap_or(dark.background),
        surface: parse_theme_color(t.get("surface")).unwrap_or(dark.surface),
        surface_alt: parse_theme_color(t.get("surface_alt")).unwrap_or(dark.surface_alt),
        text: parse_theme_color(t.get("text")).unwrap_or(dark.text),
        muted_text: parse_theme_color(t.get("muted_text")).unwrap_or(dark.muted_text),
        accent: parse_theme_color(t.get("accent")).unwrap_or(dark.accent),
        border: parse_theme_color(t.get("border")).unwrap_or(dark.border),
        danger: parse_theme_color(t.get("danger")).unwrap_or(dark.danger),
        warning: parse_theme_color(t.get("warning")).unwrap_or(dark.warning),
        success: parse_theme_color(t.get("success")).unwrap_or(dark.success),
        focus: parse_theme_color(t.get("focus")).unwrap_or(dark.focus),
        disabled: parse_theme_color(t.get("disabled")).unwrap_or(dark.disabled),
        radius: t
            .get("radius")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(dark.radius),
        spacing: t
            .get("spacing")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(dark.spacing),
        font_size: t
            .get("font_size")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(dark.font_size),
    })
}

fn parse_theme_color(v: Option<&serde_json::Value>) -> Option<[f32; 4]> {
    parse_web_color(v?.as_str()?)
}

fn parse_color_ref(v: Option<&serde_json::Value>) -> Option<ColorRef> {
    match v? {
        Value::String(s) => parse_web_color(s)
            .map(ColorRef::Rgba)
            .or_else(|| Some(ColorRef::Token(s.trim().to_string()))),
        Value::Array(items) if items.len() == 3 || items.len() == 4 => {
            let r = items.first()?.as_f64()? as f32;
            let g = items.get(1)?.as_f64()? as f32;
            let b = items.get(2)?.as_f64()? as f32;
            let a = items.get(3).and_then(Value::as_f64).unwrap_or(1.0) as f32;
            Some(ColorRef::Rgba([
                normalize_color_channel(r),
                normalize_color_channel(g),
                normalize_color_channel(b),
                a.clamp(0.0, 1.0),
            ]))
        }
        _ => None,
    }
}

fn normalize_color_channel(value: f32) -> f32 {
    if value > 1.0 {
        (value / 255.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn parse_line_plot_series(props: &serde_json::Value) -> Vec<LinePlotSeriesProp> {
    props
        .get("series")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(parse_line_plot_series_item)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_line_plot_series_item(value: &serde_json::Value) -> Option<LinePlotSeriesProp> {
    let obj = value.as_object()?;
    let payload_format = obj
        .get("data_format")
        .and_then(Value::as_str)
        .map(LinePlotPayloadFormat::from_str)
        .unwrap_or_default();
    let points = obj
        .get("data_b64")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|data| match decode_line_plot_xy_b64(data) {
            Ok(points) => points,
            Err(err) => {
                eprintln!("DragonGUI: line plot data decode: {err}");
                Vec::new()
            }
        })
        .unwrap_or_default();
    let bounds = line_plot_points_bounds(&points);
    Some(LinePlotSeriesProp {
        label: obj
            .get("label")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        color: parse_color_ref(obj.get("color")),
        line_style: parse_line_plot_line_style(obj.get("line_style").and_then(Value::as_str)),
        points,
        bounds,
        payload_format,
        declared_point_count: obj
            .get("points")
            .and_then(Value::as_u64)
            .map(|v| v as usize),
    })
}

pub(crate) fn line_plot_points_bounds(points: &[[f32; 2]]) -> Option<[f32; 4]> {
    let mut x_min = f32::INFINITY;
    let mut x_max = f32::NEG_INFINITY;
    let mut y_min = f32::INFINITY;
    let mut y_max = f32::NEG_INFINITY;
    let mut has_point = false;
    for [x, y] in points {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        x_min = x_min.min(*x);
        x_max = x_max.max(*x);
        y_min = y_min.min(*y);
        y_max = y_max.max(*y);
        has_point = true;
    }
    has_point.then_some([x_min, x_max, y_min, y_max])
}

fn parse_f32_vec(v: Option<&serde_json::Value>) -> Vec<f32> {
    v.and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_f64)
                .filter(|value| value.is_finite())
                .map(|value| value as f32)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn parse_histogram_props(props: &serde_json::Value) -> HistogramProp {
    let mut edges = parse_f32_vec(props.get("edges"));
    let mut counts = parse_f32_vec(props.get("counts"));
    if edges.len() != counts.len().saturating_add(1) {
        edges.clear();
        counts.clear();
    }
    if edges
        .windows(2)
        .any(|pair| !pair[0].is_finite() || !pair[1].is_finite() || pair[1] <= pair[0])
    {
        edges.clear();
        counts.clear();
    }
    let mode = props
        .get("mode")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "count" | "density" | "probability" | "percent"))
        .unwrap_or("count")
        .to_string();
    HistogramProp {
        edges,
        counts,
        color: parse_color_ref(props.get("color")),
        label: props
            .get("label")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        x_label: props
            .get("x_label")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        y_label: props
            .get("y_label")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        mode,
        cumulative: props
            .get("cumulative")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        show_grid: props
            .get("show_grid")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        show_axes: props
            .get("show_axes")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        show_ticks: props
            .get("show_ticks")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        show_toolbar: props
            .get("show_toolbar")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        tick_count: props
            .get("tick_count")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(5)
            .clamp(2, 9),
        auto_fit: props
            .get("auto_fit")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        x_min: props
            .get("x_min")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .map(|value| value as f32),
        x_max: props
            .get("x_max")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .map(|value| value as f32),
        y_min: props
            .get("y_min")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .map(|value| value as f32),
        y_max: props
            .get("y_max")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .map(|value| value as f32),
        interaction: props
            .get("interaction")
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "inspect" | "pan" | "zoom" | "box_zoom"))
            .unwrap_or("inspect")
            .to_string(),
        selection_rect: None,
        bar_gap: props
            .get("bar_gap")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| value as f32)
            .unwrap_or(1.0),
    }
}

pub(crate) fn parse_line_plot_line_style(value: Option<&str>) -> String {
    let normalized = value
        .unwrap_or("solid")
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-");
    match normalized.as_str() {
        "dash" => "dashed".to_string(),
        "dot" => "dotted".to_string(),
        "dash-dot" => "dashdot".to_string(),
        "solid" | "dashed" | "dotted" | "dashdot" => normalized,
        _ => "solid".to_string(),
    }
}

fn decode_line_plot_xy_b64(data: &str) -> Result<Vec<[f32; 2]>, String> {
    let bytes = BASE64.decode(data).map_err(|e| format!("base64: {e}"))?;
    if bytes.len() % 8 != 0 {
        return Err(format!(
            "payload length {} is not a multiple of 8 (xy float32)",
            bytes.len()
        ));
    }
    Ok(bytes
        .chunks_exact(8)
        .map(|chunk| {
            [
                f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]),
            ]
        })
        .collect())
}

pub fn parse_stylesheets_from_doc(doc: &serde_json::Value) -> StylesheetStore {
    let mut store = StylesheetStore::default();
    let Some(stylesheets) = doc.get("stylesheets").and_then(|value| value.as_array()) else {
        return store;
    };
    for stylesheet in stylesheets {
        let source = stylesheet
            .get("source")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if source.trim().is_empty() {
            continue;
        }
        let origin = match stylesheet
            .get("origin")
            .and_then(|value| value.as_str())
            .unwrap_or("user")
        {
            "framework" => StylesheetOrigin::Framework,
            "theme" => StylesheetOrigin::Theme,
            "user" => StylesheetOrigin::User,
            other => {
                eprintln!("DragonGUI: ignoring stylesheet with unsupported origin {other:?}");
                continue;
            }
        };
        if let Err(error) = store.set_stylesheet(origin, source) {
            eprintln!("DragonGUI: ignoring invalid stylesheet: {error}");
        }
    }
    store
}

// ---------------------------------------------------------------------------
// Full widget tree — used by the layout engine.
// ---------------------------------------------------------------------------

/// Widget type.  Unknown variants are preserved so unrecognised widgets still
/// contribute a flex-grow leaf to the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    Window,
    HLayout,
    VLayout,
    ScrollArea,
    GridLayout,
    FlowLayout,
    Panel,
    Collapsible,
    Modal,
    Badge,
    Tag,
    Led,
    Button,
    Checkbox,
    Dropdown,
    Label,
    Slider,
    NumberInput,
    ProgressBar,
    TextInput,
    TextArea,
    Separator,
    Spacer,
    StatusBar,
    MenuBar,
    Menu,
    MenuItem,
    ContextMenu,
    Tooltip,
    Toast,
    Tabs,
    Tab,
    Pages,
    Page,
    Sidebar,
    NavItem,
    Histogram,
    LinePlot,
    Scatter3D,
    DataFrameTable,
    HtmlReport,
    Image,
    Unknown,
}

impl WidgetKind {
    fn from_str(s: &str) -> Self {
        match s {
            "window" => WidgetKind::Window,
            "h_layout" => WidgetKind::HLayout,
            "v_layout" => WidgetKind::VLayout,
            "scroll_area" => WidgetKind::ScrollArea,
            "grid_layout" => WidgetKind::GridLayout,
            "flow_layout" => WidgetKind::FlowLayout,
            "panel" => WidgetKind::Panel,
            "collapsible" => WidgetKind::Collapsible,
            "modal" => WidgetKind::Modal,
            "badge" => WidgetKind::Badge,
            "tag" => WidgetKind::Tag,
            "led" => WidgetKind::Led,
            "button" => WidgetKind::Button,
            "checkbox" => WidgetKind::Checkbox,
            "dropdown" => WidgetKind::Dropdown,
            "label" => WidgetKind::Label,
            "slider" => WidgetKind::Slider,
            "number_input" => WidgetKind::NumberInput,
            "progress_bar" => WidgetKind::ProgressBar,
            "text_input" => WidgetKind::TextInput,
            "text_area" => WidgetKind::TextArea,
            "separator" => WidgetKind::Separator,
            "spacer" => WidgetKind::Spacer,
            "status_bar" => WidgetKind::StatusBar,
            "menu_bar" => WidgetKind::MenuBar,
            "menu" => WidgetKind::Menu,
            "menu_item" => WidgetKind::MenuItem,
            "context_menu" => WidgetKind::ContextMenu,
            "tooltip" => WidgetKind::Tooltip,
            "toast" => WidgetKind::Toast,
            "tabs" => WidgetKind::Tabs,
            "tab" => WidgetKind::Tab,
            "pages" => WidgetKind::Pages,
            "page" => WidgetKind::Page,
            "sidebar" => WidgetKind::Sidebar,
            "nav_item" => WidgetKind::NavItem,
            "histogram" => WidgetKind::Histogram,
            "line_plot" => WidgetKind::LinePlot,
            "scatter_3d" => WidgetKind::Scatter3D,
            "dataframe_table" => WidgetKind::DataFrameTable,
            "html_report" => WidgetKind::HtmlReport,
            "image" => WidgetKind::Image,
            _ => WidgetKind::Unknown,
        }
    }
}

/// Layout-relevant properties extracted from each node's `props` object.
#[derive(Debug, Clone, Default)]
pub struct NodeProps {
    /// Original serialized props for CSS generated content attr(...) lookups.
    pub raw_props: Map<String, Value>,
    /// Fixed pixel width (Panel with `width` set).
    pub fixed_width: Option<f32>,
    /// Fixed pixel height.
    pub fixed_height: Option<f32>,
    /// GridLayout: fixed column count (None → auto-fill).
    pub grid_columns: Option<u16>,
    /// GridLayout: minimum column width in logical pixels for minmax tracks.
    pub grid_min_column_width: Option<f32>,
    /// GridLayout: explicit column track template.
    pub grid_template_columns: Option<Vec<GridTrackSize>>,
    /// GridLayout: explicit row track template.
    pub grid_template_rows: Option<Vec<GridTrackSize>>,
    /// FlowLayout main-axis alignment: start, center, or end.
    pub flow_align: Option<String>,
    /// FlowLayout cross-axis alignment: start, center, end, or stretch.
    pub flow_cross_align: Option<String>,
    /// Explicit or auto layout orientation for separators.
    pub orientation: Option<String>,
    /// Checkbox initial checked state.
    pub checked: Option<bool>,
    /// Slider current value.
    pub value: Option<f32>,
    /// Slider minimum value.
    pub min: Option<f32>,
    /// Slider maximum value.
    pub max: Option<f32>,
    /// Slider keyboard step.
    pub step: Option<f32>,
    /// Display text for Label and Button widgets; label for Checkbox.
    pub text: Option<String>,
    /// Optional inline badge text for selected controls.
    pub badge: Option<String>,
    /// Semantic level for standalone Badge/Tag widgets.
    pub level: Option<String>,
    /// Placeholder text for TextInput.
    pub placeholder: Option<String>,
    /// Preferred visible row count for TextArea.
    pub rows: Option<u32>,
    /// Whether TextArea wraps long lines.
    pub wrap: Option<bool>,
    /// Dropdown choices.
    pub items: Vec<String>,
    /// Stable navigation key for Tabs, Tab, Pages, and Page nodes.
    pub route_value: Option<String>,
    /// Target page key for NavItem nodes.
    pub page: Option<String>,
    /// DataFrame table column names.
    pub table_columns: Vec<String>,
    /// DataFrame table column dtype labels.
    pub table_dtypes: Vec<String>,
    /// DataFrame table row count.
    pub table_rows: Option<usize>,
    /// Retained native table resource id.
    pub table_resource_id: Option<String>,
    /// Number of formatted rows included in the initial resource payload.
    pub table_sample_rows: Option<usize>,
    /// Requested visible row window size from Python.
    pub page_size: Option<usize>,
    /// Bounded startup sample of formatted table cells, row-major.
    pub table_cells: Vec<Vec<String>>,
    /// Non-interactive disabled state.
    pub disabled: bool,
    /// Collapsible container expanded state.
    pub expanded: Option<bool>,
    /// Modal visibility state.
    pub open: Option<bool>,
    /// Context menu target widget id.
    pub target: Option<String>,
    /// Optional static tooltip text shown on hover.
    pub tooltip: Option<String>,
    /// Image file path for native image display widgets.
    pub image_path: Option<String>,
    /// Image fit mode: contain, cover, or stretch.
    pub image_fit: Option<String>,
    /// Local HTML report file path for future embedded webview backends.
    pub html_report_path: Option<String>,
    /// Inline HTML report document for future embedded webview backends.
    pub html_report_html: Option<String>,
    /// Base directory used to resolve relative resources for inline HTML reports.
    pub html_report_base_dir: Option<String>,
    /// Whether remote subresources are allowed when an embedded webview is active.
    pub html_report_allow_remote: bool,
    /// Whether scripts are allowed when an embedded webview is active.
    pub html_report_allow_scripts: bool,
    /// Whether opening in the system browser is an acceptable fallback.
    pub html_report_external_fallback: bool,
    /// LED status state name and resolved indicator color.
    pub led_state: Option<String>,
    pub led_color: Option<ColorRef>,
    pub led_size: Option<f32>,
    /// LinePlot packed startup series.
    pub line_plot_series: Vec<LinePlotSeriesProp>,
    pub line_plot_x_label: Option<String>,
    pub line_plot_y_label: Option<String>,
    pub line_plot_show_grid: bool,
    pub line_plot_show_axes: bool,
    pub line_plot_show_ticks: bool,
    pub line_plot_show_toolbar: bool,
    pub line_plot_show_legend: bool,
    pub line_plot_legend_position: String,
    pub line_plot_tick_count: usize,
    pub line_plot_auto_fit: bool,
    pub line_plot_line_width: f32,
    pub line_plot_window_size: Option<f32>,
    pub line_plot_x_min: Option<f32>,
    pub line_plot_x_max: Option<f32>,
    pub line_plot_y_min: Option<f32>,
    pub line_plot_y_max: Option<f32>,
    pub line_plot_interaction: String,
    pub line_plot_selection_rect: Option<[f32; 4]>,
    pub line_plot_hover: Option<LinePlotHoverProp>,
    /// Histogram pre-binned startup data and chrome props.
    pub histogram: HistogramProp,
    /// Scatter3D colormap name.
    pub scatter_colormap: Option<String>,
    /// Scatter3D base64-encoded startup payload.
    pub scatter_data_b64: Option<String>,
    /// Scatter3D payload wire format.
    pub scatter_data_format: ScatterPayloadFormat,
    /// Scatter3D grid/chrome startup props.
    pub scatter_grid_visible: bool,
    pub scatter_major_planes: bool,
    pub scatter_minor_planes: bool,
    pub scatter_grid_sticky: bool,
    pub scatter_grid_all_edges: bool,
    pub scatter_tick_override: [Option<usize>; 3],
    pub scatter_axis_labels: [String; 3],
    pub scatter_axis_visible: [bool; 3],
    pub scatter_background: Option<[f32; 4]>,
    /// Scatter3D legend startup props.
    pub scatter_legend_visible: bool,
    pub scatter_legend_position: String,
    pub scatter_legend_entries: Vec<(String, f32, f32, f32)>,
    pub scatter_legend_title: Option<String>,
    /// Scatter3D scalar bar startup props.
    pub scatter_scalar_bar_visible: bool,
    pub scatter_scalar_bar_vmin: f32,
    pub scatter_scalar_bar_vmax: f32,
    pub scatter_scalar_bar_log_scale: bool,
    pub scatter_scalar_bar_colormap: String,
    pub scatter_scalar_bar_title: Option<String>,
    /// Scatter3D orientation axes startup prop.
    pub scatter_orientation_axes_visible: bool,
}

/// One node in the widget tree.
#[derive(Debug, Clone)]
pub struct WidgetNode {
    pub id: String,
    /// Stable identity metadata for the future reactive retained tree.
    #[allow(dead_code)]
    pub key: Option<String>,
    /// Semantic class label for debug snapshots and future stylesheet targeting.
    #[allow(dead_code)]
    pub class_name: Option<String>,
    pub kind: WidgetKind,
    pub props: NodeProps,
    /// Raw structured style map retained so live style patches can merge and
    /// reparse computed style without rebuilding the whole document.
    pub style_json: Map<String, Value>,
    /// Parsed inline style, kept separate from the computed stylesheet result.
    pub inline_style: NodeStyle,
    pub style: NodeStyle,
    pub children: Vec<WidgetNode>,
}

/// Parse the `"window"` subtree of the top-level app document into a
/// `WidgetNode` tree.  Returns `None` only if the JSON lacks a `"window"` key.
pub fn parse_widget_tree(doc: &serde_json::Value) -> Option<WidgetNode> {
    let window = doc.get("window")?;
    parse_widget_node(window)
}

pub fn parse_widget_node(v: &serde_json::Value) -> Option<WidgetNode> {
    let id = v
        .get("id")
        .and_then(|i| i.as_str())
        .unwrap_or("_anon")
        .to_string();
    let key = v.get("key").and_then(|k| k.as_str()).map(|s| s.to_string());
    let class_name = v
        .get("class")
        .and_then(|k| k.as_str())
        .map(|s| s.to_string());
    let kind = WidgetKind::from_str(v.get("type").and_then(|t| t.as_str()).unwrap_or(""));
    let props_val = v.get("props").unwrap_or(&serde_json::Value::Null);
    let props = parse_props(&kind, props_val);
    let style_json = style_map_from_value(v.get("style"));
    let inline_style = NodeStyle::from_json(Some(&Value::Object(style_json.clone())));
    let style = inline_style.clone();
    let children = v
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(parse_widget_node).collect())
        .unwrap_or_default();
    Some(WidgetNode {
        id,
        key,
        class_name,
        kind,
        props,
        style_json,
        inline_style,
        style,
        children,
    })
}

pub fn style_map_from_value(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn parse_props(kind: &WidgetKind, props: &serde_json::Value) -> NodeProps {
    let raw_props = props.as_object().cloned().unwrap_or_default();
    let fixed_width = props
        .get("width")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let fixed_height = props
        .get("height")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let grid_columns = props
        .get("columns")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(256) as u16);
    let grid_min_column_width = props
        .get("min_column_width")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let grid_template_columns = props
        .get("template_columns")
        .or_else(|| props.get("grid_template_columns"))
        .and_then(parse_grid_template_tracks_value);
    let grid_template_rows = props
        .get("template_rows")
        .or_else(|| props.get("grid_template_rows"))
        .and_then(parse_grid_template_tracks_value);
    let flow_align = props
        .get("align")
        .and_then(|v| v.as_str())
        .filter(|v| matches!(*v, "start" | "center" | "end"))
        .map(|v| v.to_string());
    let flow_cross_align = props
        .get("cross_align")
        .and_then(|v| v.as_str())
        .filter(|v| matches!(*v, "start" | "center" | "end" | "stretch"))
        .map(|v| v.to_string());
    let orientation = props
        .get("orientation")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let checked = props.get("checked").and_then(|v| v.as_bool());
    let value = props
        .get("value")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let min = props.get("min").and_then(|v| v.as_f64()).map(|v| v as f32);
    let max = props.get("max").and_then(|v| v.as_f64()).map(|v| v as f32);
    let step = props.get("step").and_then(|v| v.as_f64()).map(|v| v as f32);
    let badge = props
        .get("badge")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let level = props
        .get("level")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase());
    let placeholder = props
        .get("placeholder")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let rows = props.get("rows").and_then(|v| v.as_u64()).map(|v| v as u32);
    let wrap = props.get("wrap").and_then(|v| v.as_bool());
    let items = props
        .get("items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let route_value = props
        .get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let page = props
        .get("page")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let frame = props.get("frame").unwrap_or(&serde_json::Value::Null);
    let table_columns = frame
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let table_dtypes = frame
        .get("dtypes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let table_rows = frame
        .get("rows")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let table_resource_id = props
        .get("resource_id")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let table_sample_rows = props
        .get("sample_rows")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let page_size = props
        .get("page_size")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let table_cells = props
        .get("cells")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.as_array())
                .map(|row| row.iter().map(parse_cell_value).collect())
                .collect()
        })
        .unwrap_or_default();
    let disabled = props
        .get("disabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let expanded = props.get("expanded").and_then(|v| v.as_bool());
    let open = props.get("open").and_then(|v| v.as_bool());
    let target = props
        .get("target")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let tooltip = props
        .get("tooltip")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let image_path = if matches!(kind, WidgetKind::Image) {
        props
            .get("path")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
    } else {
        None
    };
    let image_fit = if matches!(kind, WidgetKind::Image) {
        props
            .get("fit")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_ascii_lowercase())
    } else {
        None
    };
    let (
        html_report_path,
        html_report_html,
        html_report_base_dir,
        html_report_allow_remote,
        html_report_allow_scripts,
        html_report_external_fallback,
    ) = if matches!(kind, WidgetKind::HtmlReport) {
        (
            props
                .get("path")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string()),
            props
                .get("html")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
                .map(|v| v.to_string()),
            props
                .get("base_dir")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string()),
            props
                .get("allow_remote")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            props
                .get("allow_scripts")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            props
                .get("external_fallback")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        )
    } else {
        (None, None, None, false, false, false)
    };
    let led_state = props
        .get("state")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let led_color = parse_color_ref(props.get("color"));
    let led_size = props
        .get("size")
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| v as f32);
    let (
        line_plot_series,
        line_plot_x_label,
        line_plot_y_label,
        line_plot_show_grid,
        line_plot_show_axes,
        line_plot_show_ticks,
        line_plot_show_toolbar,
        line_plot_show_legend,
        line_plot_legend_position,
        line_plot_tick_count,
        line_plot_auto_fit,
        line_plot_line_width,
        line_plot_window_size,
        line_plot_x_min,
        line_plot_x_max,
        line_plot_y_min,
        line_plot_y_max,
        line_plot_interaction,
        line_plot_selection_rect,
        line_plot_hover,
    ) = if matches!(kind, WidgetKind::LinePlot) {
        let x_label = props
            .get("x_label")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let y_label = props
            .get("y_label")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let show_grid = props
            .get("show_grid")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let show_axes = props
            .get("show_axes")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let show_ticks = props
            .get("show_ticks")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let show_toolbar = props
            .get("show_toolbar")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let show_legend = props
            .get("show_legend")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let legend_position = props
            .get("legend_position")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "top-right".to_string());
        let tick_count = props
            .get("tick_count")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(5)
            .clamp(2, 9);
        let auto_fit = props
            .get("auto_fit")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let line_width = props
            .get("line_width")
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as f32)
            .unwrap_or(2.0);
        let window_size = props
            .get("window_size")
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v as f32);
        let limit = |name: &str| {
            props
                .get(name)
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .map(|v| v as f32)
        };
        let interaction = props
            .get("interaction")
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "inspect" | "pan" | "zoom" | "box_zoom"))
            .unwrap_or("inspect")
            .to_string();
        (
            parse_line_plot_series(props),
            x_label,
            y_label,
            show_grid,
            show_axes,
            show_ticks,
            show_toolbar,
            show_legend,
            legend_position,
            tick_count,
            auto_fit,
            line_width,
            window_size,
            limit("x_min"),
            limit("x_max"),
            limit("y_min"),
            limit("y_max"),
            interaction,
            None,
            None,
        )
    } else {
        (
            Vec::new(),
            None,
            None,
            true,
            true,
            true,
            false,
            false,
            "top-right".to_string(),
            5,
            true,
            2.0,
            None,
            None,
            None,
            None,
            None,
            String::new(),
            None,
            None,
        )
    };
    let histogram = if matches!(kind, WidgetKind::Histogram) {
        parse_histogram_props(props)
    } else {
        HistogramProp::default()
    };
    let (scatter_colormap, scatter_data_b64, scatter_data_format) =
        if matches!(kind, WidgetKind::Scatter3D) {
            let cmap = props
                .get("colormap")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "viridis".to_string());
            let b64 = props
                .get("data_b64")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let fmt = props
                .get("data_format")
                .and_then(|v| v.as_str())
                .map(ScatterPayloadFormat::from_str)
                .unwrap_or_default();
            (Some(cmap), b64, fmt)
        } else {
            (None, None, ScatterPayloadFormat::default())
        };
    let (
        scatter_grid_visible,
        scatter_major_planes,
        scatter_minor_planes,
        scatter_grid_sticky,
        scatter_grid_all_edges,
        scatter_tick_override,
        scatter_axis_labels,
        scatter_axis_visible,
        scatter_background,
        scatter_legend_visible,
        scatter_legend_position,
        scatter_legend_entries,
        scatter_legend_title,
        scatter_scalar_bar_visible,
        scatter_scalar_bar_vmin,
        scatter_scalar_bar_vmax,
        scatter_scalar_bar_log_scale,
        scatter_scalar_bar_colormap,
        scatter_scalar_bar_title,
        scatter_orientation_axes_visible,
    ) = if matches!(kind, WidgetKind::Scatter3D) {
        let grid_visible = props
            .get("grid_visible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let major_planes = props
            .get("major_planes")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let minor_planes = props
            .get("minor_planes")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let grid_sticky = props
            .get("grid_sticky")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let grid_all_edges = props
            .get("grid_all_edges")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let tick_override = {
            let x = props
                .get("tick_x")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let y = props
                .get("tick_y")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let z = props
                .get("tick_z")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            [x, y, z]
        };
        let axis_labels = {
            let x = props
                .get("axis_x")
                .and_then(|v| v.as_str())
                .unwrap_or("X")
                .to_string();
            let y = props
                .get("axis_y")
                .and_then(|v| v.as_str())
                .unwrap_or("Y")
                .to_string();
            let z = props
                .get("axis_z")
                .and_then(|v| v.as_str())
                .unwrap_or("Z")
                .to_string();
            [x, y, z]
        };
        let axis_visible = {
            let x = props
                .get("axis_vis_x")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let y = props
                .get("axis_vis_y")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let z = props
                .get("axis_vis_z")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            [x, y, z]
        };
        let background = props
            .get("background")
            .and_then(|v| v.as_array())
            .filter(|a| a.len() == 4)
            .map(|a| {
                [
                    a[0].as_f64().unwrap_or(0.0) as f32,
                    a[1].as_f64().unwrap_or(0.0) as f32,
                    a[2].as_f64().unwrap_or(0.0) as f32,
                    a[3].as_f64().unwrap_or(1.0) as f32,
                ]
            });
        let legend_visible = props
            .get("legend_visible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let legend_position = props
            .get("legend_position")
            .and_then(|v| v.as_str())
            .unwrap_or("top_right")
            .to_string();
        let legend_entries: Vec<(String, f32, f32, f32)> = props
            .get("legend_entries")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        let label = e.get("label")?.as_str()?.to_string();
                        let color = e.get("color")?.as_array()?;
                        if color.len() < 3 {
                            return None;
                        }
                        Some((
                            label,
                            color[0].as_f64().unwrap_or(1.0) as f32,
                            color[1].as_f64().unwrap_or(1.0) as f32,
                            color[2].as_f64().unwrap_or(1.0) as f32,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let legend_title = props
            .get("legend_title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let scalar_bar_visible = props
            .get("scalar_bar_visible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let scalar_bar_vmin = props
            .get("scalar_bar_vmin")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        let scalar_bar_vmax = props
            .get("scalar_bar_vmax")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32;
        let scalar_bar_log_scale = props
            .get("scalar_bar_log_scale")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let scalar_bar_colormap = props
            .get("scalar_bar_colormap")
            .and_then(|v| v.as_str())
            .unwrap_or("viridis")
            .to_string();
        let scalar_bar_title = props
            .get("scalar_bar_title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let orientation_axes_visible = props
            .get("orientation_axes_visible")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        (
            grid_visible,
            major_planes,
            minor_planes,
            grid_sticky,
            grid_all_edges,
            tick_override,
            axis_labels,
            axis_visible,
            background,
            legend_visible,
            legend_position,
            legend_entries,
            legend_title,
            scalar_bar_visible,
            scalar_bar_vmin,
            scalar_bar_vmax,
            scalar_bar_log_scale,
            scalar_bar_colormap,
            scalar_bar_title,
            orientation_axes_visible,
        )
    } else {
        (
            false,
            false,
            false,
            true,
            false,
            [None; 3],
            ["X".to_string(), "Y".to_string(), "Z".to_string()],
            [true; 3],
            None,
            false,
            "top_right".to_string(),
            Vec::new(),
            None,
            false,
            0.0_f32,
            1.0_f32,
            false,
            "viridis".to_string(),
            None,
            false,
        )
    };
    let text_key = match kind {
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal | WidgetKind::Collapsible => {
            "title"
        }
        WidgetKind::Checkbox => "label",
        WidgetKind::Dropdown | WidgetKind::TextInput | WidgetKind::TextArea => "value",
        WidgetKind::ProgressBar => "label",
        WidgetKind::Tab | WidgetKind::NavItem | WidgetKind::Menu | WidgetKind::MenuItem => "label",
        WidgetKind::Page => "title",
        _ => "text",
    };
    let text = props
        .get(text_key)
        .or_else(|| {
            if matches!(kind, WidgetKind::TextInput | WidgetKind::TextArea) {
                props.get("placeholder")
            } else {
                None
            }
        })
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    NodeProps {
        raw_props,
        fixed_width,
        fixed_height,
        grid_columns,
        grid_min_column_width,
        grid_template_columns,
        grid_template_rows,
        flow_align,
        flow_cross_align,
        orientation,
        checked,
        value,
        min,
        max,
        step,
        text,
        badge,
        level,
        placeholder,
        rows,
        wrap,
        items,
        route_value,
        page,
        table_columns,
        table_dtypes,
        table_rows,
        table_resource_id,
        table_sample_rows,
        page_size,
        table_cells,
        disabled,
        expanded,
        open,
        target,
        tooltip,
        image_path,
        image_fit,
        html_report_path,
        html_report_html,
        html_report_base_dir,
        html_report_allow_remote,
        html_report_allow_scripts,
        html_report_external_fallback,
        led_state,
        led_color,
        led_size,
        line_plot_series,
        line_plot_x_label,
        line_plot_y_label,
        line_plot_show_grid,
        line_plot_show_axes,
        line_plot_show_ticks,
        line_plot_show_toolbar,
        line_plot_show_legend,
        line_plot_legend_position,
        line_plot_tick_count,
        line_plot_auto_fit,
        line_plot_line_width,
        line_plot_window_size,
        line_plot_x_min,
        line_plot_x_max,
        line_plot_y_min,
        line_plot_y_max,
        line_plot_interaction,
        line_plot_selection_rect,
        line_plot_hover,
        histogram,
        scatter_colormap,
        scatter_data_b64,
        scatter_data_format,
        scatter_grid_visible,
        scatter_major_planes,
        scatter_minor_planes,
        scatter_grid_sticky,
        scatter_grid_all_edges,
        scatter_tick_override,
        scatter_axis_labels,
        scatter_axis_visible,
        scatter_background,
        scatter_legend_visible,
        scatter_legend_position,
        scatter_legend_entries,
        scatter_legend_title,
        scatter_scalar_bar_visible,
        scatter_scalar_bar_vmin,
        scatter_scalar_bar_vmax,
        scatter_scalar_bar_log_scale,
        scatter_scalar_bar_colormap,
        scatter_scalar_bar_title,
        scatter_orientation_axes_visible,
    }
}

fn parse_cell_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        _ => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_theme_accepts_web_color_syntax() {
        let doc = json!({
            "theme": {
                "background": "hsl(222, 28%, 9%)",
                "surface": "rgba(18, 25, 39, 0.92)",
                "surface_alt": "#1238",
                "text": "white",
                "muted_text": "rgb(100% 50% 0% / 40%)",
                "accent": "transparent",
                "border": "not a color",
                "radius": 12,
                "spacing": 10,
                "font_size": 15
            }
        });

        let theme = parse_theme_from_doc(&doc).expect("theme");
        assert_color_close(theme.background, [0.0648, 0.0792, 0.1152, 1.0]);
        assert_color_close(
            theme.surface,
            [18.0 / 255.0, 25.0 / 255.0, 39.0 / 255.0, 0.92],
        );
        assert_color_close(
            theme.surface_alt,
            [
                0x11 as f32 / 255.0,
                0x22 as f32 / 255.0,
                0x33 as f32 / 255.0,
                0x88 as f32 / 255.0,
            ],
        );
        assert_eq!(theme.text, [1.0, 1.0, 1.0, 1.0]);
        assert_color_close(theme.muted_text, [1.0, 0.5, 0.0, 0.4]);
        assert_eq!(theme.accent, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(theme.border, Theme::dark().border);
        assert_eq!(theme.radius, 12.0);
        assert_eq!(theme.spacing, 10.0);
        assert_eq!(theme.font_size, 15.0);
    }

    #[test]
    fn parse_widget_tree_retains_raw_style_for_live_patches() {
        let doc = json!({
            "window": {
                "id": "window",
                "type": "window",
                "props": {"title": "Styles", "width": 320, "height": 240},
                "style": {
                    "width": 320,
                    "background": "surface",
                    "hover": {"background": "accent_mix_20"}
                }
            }
        });

        let tree = parse_widget_tree(&doc).unwrap();

        assert_eq!(tree.style_json.get("background").unwrap(), "surface");
        assert_eq!(tree.style.layout.width, Some(320.0));
        assert!(tree.style.hover.background.is_some());
    }

    #[test]
    fn parse_grid_layout_template_track_props() {
        let node = parse_widget_node(&json!({
            "id": "stats",
            "type": "grid_layout",
            "props": {
                "template_columns": [44, {"fr": 1}, {"minmax": {"min": 72, "max": {"fr": 1}}}],
                "template_rows": "18px auto"
            }
        }))
        .unwrap();

        assert_eq!(
            node.props.grid_template_columns,
            Some(vec![
                GridTrackSize::LogicalPx(44.0),
                GridTrackSize::Fraction(1.0),
                GridTrackSize::MinMax {
                    min: crate::style::GridTrackMinSize::LogicalPx(72.0),
                    max: crate::style::GridTrackMaxSize::Fraction(1.0),
                },
            ])
        );
        assert_eq!(
            node.props.grid_template_rows,
            Some(vec![GridTrackSize::LogicalPx(18.0), GridTrackSize::Auto])
        );
    }

    #[test]
    fn parse_image_widget_props() {
        let doc = json!({
            "window": {
                "id": "window",
                "type": "window",
                "props": {"title": "Images", "width": 320, "height": 240},
                "children": [{
                    "id": "hero",
                    "type": "image",
                    "props": {
                        "path": "examples/demo.png",
                        "fit": "cover",
                        "width": 160,
                        "height": 90
                    }
                }]
            }
        });

        let tree = parse_widget_tree(&doc).unwrap();
        let image = &tree.children[0];

        assert_eq!(image.kind, WidgetKind::Image);
        assert_eq!(image.props.image_path.as_deref(), Some("examples/demo.png"));
        assert_eq!(image.props.image_fit.as_deref(), Some("cover"));
        assert_eq!(image.props.fixed_width, Some(160.0));
        assert_eq!(image.props.fixed_height, Some(90.0));
    }

    #[test]
    fn parse_led_widget_props() {
        let doc = json!({
            "window": {
                "id": "window",
                "type": "window",
                "props": {"title": "LED", "width": 320, "height": 240},
                "children": [{
                    "id": "status",
                    "type": "led",
                    "props": {
                        "state": "busy",
                        "color": "#ffcc33",
                        "size": 18
                    }
                }]
            }
        });

        let tree = parse_widget_tree(&doc).unwrap();
        let led = &tree.children[0];

        assert_eq!(led.kind, WidgetKind::Led);
        assert_eq!(led.props.led_state.as_deref(), Some("busy"));
        assert_eq!(led.props.led_size, Some(18.0));
        assert_color_close(
            led.props
                .led_color
                .as_ref()
                .unwrap()
                .resolve(&Theme::dark()),
            [1.0, 0.8, 0x33 as f32 / 255.0, 1.0],
        );
    }

    #[test]
    fn parse_histogram_widget_props() {
        let node = parse_widget_node(&json!({
            "id": "hist",
            "type": "histogram",
            "props": {
                "edges": [0.0, 1.0, 2.0, 3.0],
                "counts": [2.0, 5.0, 1.0],
                "color": "#42a5ff",
                "label": "Latency",
                "x_label": "ms",
                "y_label": "count",
                "mode": "count",
                "cumulative": false,
                "show_grid": false,
                "show_axes": true,
                "show_ticks": false,
                "show_toolbar": true,
                "auto_fit": false,
                "x_min": 0.0,
                "x_max": 3.0,
                "y_min": 0.0,
                "y_max": 6.0,
                "interaction": "zoom",
                "tick_count": 7,
                "bar_gap": 2.0
            }
        }))
        .unwrap();

        assert_eq!(node.kind, WidgetKind::Histogram);
        assert_eq!(node.props.histogram.edges, vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(node.props.histogram.counts, vec![2.0, 5.0, 1.0]);
        assert_eq!(node.props.histogram.label.as_deref(), Some("Latency"));
        assert_eq!(node.props.histogram.x_label.as_deref(), Some("ms"));
        assert_eq!(node.props.histogram.y_label.as_deref(), Some("count"));
        assert!(!node.props.histogram.show_grid);
        assert!(!node.props.histogram.show_ticks);
        assert!(node.props.histogram.show_toolbar);
        assert!(!node.props.histogram.auto_fit);
        assert_eq!(node.props.histogram.x_min, Some(0.0));
        assert_eq!(node.props.histogram.x_max, Some(3.0));
        assert_eq!(node.props.histogram.y_min, Some(0.0));
        assert_eq!(node.props.histogram.y_max, Some(6.0));
        assert_eq!(node.props.histogram.interaction, "zoom");
        assert_eq!(node.props.histogram.tick_count, 7);
        assert_eq!(node.props.histogram.bar_gap, 2.0);
    }

    fn assert_color_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 0.003,
                "expected {expected}, got {actual}"
            );
        }
    }

    #[test]
    fn parse_stylesheets_from_doc_populates_user_store() {
        let doc = json!({
            "stylesheets": [{
                "origin": "user",
                "source": ":root { --radius: 4px; } Button { border-radius: var(--radius); }"
            }],
            "window": {
                "id": "window",
                "type": "window",
                "props": {"title": "Styles", "width": 320, "height": 240}
            }
        });

        let store = parse_stylesheets_from_doc(&doc);

        assert_eq!(store.rules(StylesheetOrigin::User).len(), 1);
        assert_eq!(
            store.variables().get("--radius"),
            Some(&crate::css_style::DgCssValue::Length(
                crate::css_style::DgCssLength::LogicalPx(4.0)
            ))
        );
    }
}
