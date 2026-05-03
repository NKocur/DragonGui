use serde::Deserialize;
use serde_json::{Map, Value};

use crate::css_style::{StylesheetOrigin, StylesheetStore};
use crate::style::NodeStyle;
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
    Scatter3D,
    DataFrameTable,
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
            "scatter_3d" => WidgetKind::Scatter3D,
            "dataframe_table" => WidgetKind::DataFrameTable,
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
    let image_path = props
        .get("path")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let image_fit = props
        .get("fit")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase());
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
