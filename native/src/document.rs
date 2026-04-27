use serde::Deserialize;
use serde_json::{Map, Value};

use crate::css_style::{StylesheetOrigin, StylesheetStore};
use crate::style::NodeStyle;
use crate::theme::{parse_hex_color as parse_hex_color_str, Theme};

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
// Scatter spec — kept for the base64 data path.
// ---------------------------------------------------------------------------

/// Column hints and optional pre-packed point data from a Python
/// `Scatter3D(df, x=..., y=..., z=...)` node.
#[derive(Debug)]
pub struct ScatterSpec {
    pub colormap: String,
    /// Base64-encoded packed float32 xyz triples supplied by Python.
    pub data_b64: Option<String>,
}

/// Return the first `scatter_3d` widget found anywhere in the document tree.
pub fn find_scatter_in_doc(v: &serde_json::Value) -> Option<ScatterSpec> {
    find_scatter_value(v)
}

fn find_scatter_value(v: &serde_json::Value) -> Option<ScatterSpec> {
    if v.get("type").and_then(|t| t.as_str()) == Some("scatter_3d") {
        let props = v.get("props")?;
        return Some(ScatterSpec {
            colormap: props
                .get("colormap")
                .and_then(|v| v.as_str())
                .unwrap_or("viridis")
                .trim()
                .to_ascii_lowercase(),
            data_b64: props
                .get("data_b64")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        });
    }
    for key in &["children", "window"] {
        match v.get(key) {
            Some(serde_json::Value::Array(arr)) => {
                for item in arr {
                    if let Some(s) = find_scatter_value(item) {
                        return Some(s);
                    }
                }
            }
            Some(obj @ serde_json::Value::Object(_)) => {
                if let Some(s) = find_scatter_value(obj) {
                    return Some(s);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Python theme parsing
// ---------------------------------------------------------------------------

/// Parse a `Theme` from the top-level `"theme"` key in the app document.
///
/// Returns `None` if no `"theme"` key is present; callers fall back to
/// `Theme::dark()`.  Unknown or malformed tokens are silently replaced with
/// the corresponding dark-theme default.
pub fn parse_theme_from_doc(doc: &serde_json::Value) -> Option<Theme> {
    let t = doc.get("theme")?;
    let dark = Theme::dark();
    Some(Theme {
        background: parse_hex_color(t.get("background")).unwrap_or(dark.background),
        surface: parse_hex_color(t.get("surface")).unwrap_or(dark.surface),
        surface_alt: parse_hex_color(t.get("surface_alt")).unwrap_or(dark.surface_alt),
        text: parse_hex_color(t.get("text")).unwrap_or(dark.text),
        muted_text: parse_hex_color(t.get("muted_text")).unwrap_or(dark.muted_text),
        accent: parse_hex_color(t.get("accent")).unwrap_or(dark.accent),
        border: parse_hex_color(t.get("border")).unwrap_or(dark.border),
        danger: parse_hex_color(t.get("danger")).unwrap_or(dark.danger),
        warning: parse_hex_color(t.get("warning")).unwrap_or(dark.warning),
        success: parse_hex_color(t.get("success")).unwrap_or(dark.success),
        focus: parse_hex_color(t.get("focus")).unwrap_or(dark.focus),
        disabled: parse_hex_color(t.get("disabled")).unwrap_or(dark.disabled),
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

fn parse_hex_color(v: Option<&serde_json::Value>) -> Option<[f32; 4]> {
    parse_hex_color_str(v?.as_str()?)
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
    Panel,
    Collapsible,
    Modal,
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
            "panel" => WidgetKind::Panel,
            "collapsible" => WidgetKind::Collapsible,
            "modal" => WidgetKind::Modal,
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
    /// Fixed pixel width (Panel with `width` set).
    pub fixed_width: Option<f32>,
    /// Fixed pixel height.
    pub fixed_height: Option<f32>,
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
    let fixed_width = props
        .get("width")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let fixed_height = props
        .get("height")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
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
        fixed_width,
        fixed_height,
        orientation,
        checked,
        value,
        min,
        max,
        step,
        text,
        badge,
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
