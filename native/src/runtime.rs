use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use pyo3::prelude::*;
use serde_json::{json, Map, Value};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalPosition;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::commands::{
    Command, CommandBridge, CommandValue, Dirty, RuntimeEvent, ScatterTelemetry, TableColumnPacket,
};
use crate::document::{self, NodeProps, ScatterSpec, WidgetKind, WidgetNode};
use crate::error::DragonError;
use crate::events::{hit_test, ChangeValue, SliderDrag, WidgetState};
use crate::layout::compute_layout;
use crate::primitives::PrimitivesRenderer;
use crate::resources::ResourceRegistry;
use crate::scatter::{self, PointInstance, ScatterWidget};
use crate::style::NodeStyle;
use crate::table::{self, TableHit};
use crate::text::TextRendererDg;
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// AppSpec — bundles everything parsed from the Python document
// ---------------------------------------------------------------------------

pub struct AppSpec {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub scatter: Option<ScatterSpec>,
    pub widget_tree: Option<WidgetNode>,
    /// Python-provided theme overrides; `None` → use `Theme::dark()` defaults.
    pub theme: Option<Theme>,
    /// Button on_click callbacks keyed by widget id.
    pub click_callbacks: HashMap<String, Box<dyn Fn() + Send>>,
    /// Checkbox / Slider on_change callbacks keyed by widget id.
    pub change_callbacks: HashMap<String, Box<dyn Fn(ChangeValue) + Send>>,
    /// Optional runtime bridge for live Python/Rust commands.
    pub command_bridge: Option<Arc<CommandBridge>>,
    /// Python AppHandle used only for draining `app.call_soon_threadsafe` tasks.
    pub python_runtime: Option<Py<PyAny>>,
}

// ---------------------------------------------------------------------------
// Public result returned to app.rs after the event loop exits
// ---------------------------------------------------------------------------

pub struct RunResult {
    pub upload_ms: f64,
    pub frame_ms: f64,
    pub debug_snapshot: String,
}

// ---------------------------------------------------------------------------
// Point cloud sources
// ---------------------------------------------------------------------------

const DEMO_POINT_COUNT: usize = 500_000;
const MAX_COMMAND_DRAIN_BATCHES: usize = 16;

fn gen_demo_points_with_colormap(colormap: &str) -> Vec<PointInstance> {
    let cmap = scatter::colormap::resolve(colormap);
    let mut pts = Vec::with_capacity(DEMO_POINT_COUNT);
    let mut seed: u64 = 0xDEAD_BEEF_1234_5678;
    let mut lcg = move || -> f32 {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 32) as u32 as f32 / u32::MAX as f32
    };
    for i in 0..DEMO_POINT_COUNT {
        let t = i as f32 / DEMO_POINT_COUNT as f32;
        let x = lcg() * 10.0 - 5.0;
        let y = lcg() * 10.0 - 5.0;
        let z = lcg() * 10.0 - 5.0;
        let [r, g, b] = scatter::colormap::sample(cmap, t);
        pts.push(PointInstance {
            position: [x, y, z],
            size: 3.0,
            color: [r, g, b],
            alpha: 0.85,
        });
    }
    pts
}

fn decode_scatter_points(b64: &str, colormap: &str) -> Result<Vec<PointInstance>, DragonError> {
    let bytes = BASE64
        .decode(b64)
        .map_err(|e| DragonError::ParseError(format!("scatter data base64: {e}")))?;
    let mut pts = Vec::new();
    decode_scatter_points_bytes_into_colormap(&bytes, &mut pts, colormap)?;
    Ok(pts)
}

fn decode_scatter_points_bytes_into_colormap(
    bytes: &[u8],
    pts: &mut Vec<PointInstance>,
    colormap: &str,
) -> Result<(), DragonError> {
    if bytes.len() % 12 != 0 {
        return Err(DragonError::ParseError(format!(
            "scatter data length {} is not a multiple of 12 (xyz float32)",
            bytes.len()
        )));
    }

    let n = bytes.len() / 12;
    let cmap = scatter::colormap::resolve(colormap);
    pts.clear();
    pts.reserve(n);

    for i in 0..n {
        let off = i * 12;
        let x = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let y = f32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
        let z = f32::from_le_bytes(bytes[off + 8..off + 12].try_into().unwrap());
        let t = i as f32 / n as f32;
        let [r, g, b] = scatter::colormap::sample(cmap, t);
        pts.push(PointInstance {
            position: [x, y, z],
            size: 3.0,
            color: [r, g, b],
            alpha: 0.85,
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Depth texture helpers
// ---------------------------------------------------------------------------

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

// ---------------------------------------------------------------------------
// Walk helpers
// ---------------------------------------------------------------------------

fn find_visible_scatter_id<'a>(
    node: &'a WidgetNode,
    layout: &crate::layout::LayoutResult,
) -> Option<&'a str> {
    if node.kind == WidgetKind::Scatter3D && layout.rects.contains_key(&node.id) {
        return Some(&node.id);
    }
    for child in &node.children {
        if let Some(id) = find_visible_scatter_id(child, layout) {
            return Some(id);
        }
    }
    None
}

fn find_first_widget_kind_id<'a>(node: &'a WidgetNode, kind: &WidgetKind) -> Option<&'a str> {
    if &node.kind == kind {
        return Some(&node.id);
    }
    for child in &node.children {
        if let Some(id) = find_first_widget_kind_id(child, kind) {
            return Some(id);
        }
    }
    None
}

fn collect_widget_kinds(node: &WidgetNode, out: &mut HashMap<String, WidgetKind>) {
    out.insert(node.id.clone(), node.kind.clone());
    for child in &node.children {
        collect_widget_kinds(child, out);
    }
}

fn find_widget_mut<'a>(node: &'a mut WidgetNode, id: &str) -> Option<&'a mut WidgetNode> {
    if node.id == id {
        return Some(node);
    }
    for child in &mut node.children {
        if let Some(found) = find_widget_mut(child, id) {
            return Some(found);
        }
    }
    None
}

fn replace_widget_children(node: &mut WidgetNode, id: &str, children: Vec<WidgetNode>) -> bool {
    if let Some(target) = find_widget_mut(node, id) {
        target.children = children;
        return true;
    }
    false
}

fn replace_widget_node(node: &mut WidgetNode, id: &str, replacement: WidgetNode) -> bool {
    if node.id == id {
        *node = replacement;
        return true;
    }
    for child in &mut node.children {
        if replace_widget_node(child, id, replacement.clone()) {
            return true;
        }
    }
    false
}

fn parse_widget_children_json(children_json: &str) -> Result<Vec<WidgetNode>, DragonError> {
    let value: Value = serde_json::from_str(children_json)
        .map_err(|e| DragonError::Runtime(format!("invalid replacement children JSON: {e}")))?;
    let Some(items) = value.as_array() else {
        return Err(DragonError::Runtime(
            "replacement children JSON must be an array".to_string(),
        ));
    };
    let mut children = Vec::with_capacity(items.len());
    for item in items {
        let node = document::parse_widget_node(item).ok_or_else(|| {
            DragonError::Runtime("replacement child is not a valid widget node".to_string())
        })?;
        children.push(node);
    }
    Ok(children)
}

fn parse_widget_node_json(node_json: &str) -> Result<WidgetNode, DragonError> {
    let value: Value = serde_json::from_str(node_json)
        .map_err(|e| DragonError::Runtime(format!("invalid replacement node JSON: {e}")))?;
    document::parse_widget_node(&value).ok_or_else(|| {
        DragonError::Runtime("replacement node is not a valid widget node".to_string())
    })
}

fn parse_table_update_json(id: &str, table_json: &str) -> Result<NodeProps, DragonError> {
    let props: Value = serde_json::from_str(table_json)
        .map_err(|e| DragonError::Runtime(format!("invalid table update JSON: {e}")))?;
    let node_value = json!({
        "id": id,
        "type": "dataframe_table",
        "props": props,
    });
    let node = document::parse_widget_node(&node_value)
        .ok_or_else(|| DragonError::Runtime("table update is not valid table data".to_string()))?;
    Ok(node.props)
}

fn widget_kind_name(kind: &WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Window => "window",
        WidgetKind::HLayout => "h_layout",
        WidgetKind::VLayout => "v_layout",
        WidgetKind::Panel => "panel",
        WidgetKind::Button => "button",
        WidgetKind::Checkbox => "checkbox",
        WidgetKind::Dropdown => "dropdown",
        WidgetKind::Label => "label",
        WidgetKind::Slider => "slider",
        WidgetKind::TextInput => "text_input",
        WidgetKind::Separator => "separator",
        WidgetKind::Spacer => "spacer",
        WidgetKind::StatusBar => "status_bar",
        WidgetKind::Tabs => "tabs",
        WidgetKind::Tab => "tab",
        WidgetKind::Pages => "pages",
        WidgetKind::Page => "page",
        WidgetKind::Sidebar => "sidebar",
        WidgetKind::NavItem => "nav_item",
        WidgetKind::Scatter3D => "scatter_3d",
        WidgetKind::DataFrameTable => "dataframe_table",
        WidgetKind::Unknown => "unknown",
    }
}

fn rect_json(rect: crate::layout::Rect) -> Value {
    json!({
        "x": rect.x,
        "y": rect.y,
        "w": rect.w,
        "h": rect.h,
    })
}

fn color_json(color: [f32; 4]) -> Value {
    json!([color[0], color[1], color[2], color[3]])
}

fn props_snapshot(node: &WidgetNode) -> Value {
    let props = &node.props;
    json!({
        "text": props.text.as_deref(),
        "fixed_width": props.fixed_width,
        "fixed_height": props.fixed_height,
        "disabled": props.disabled,
        "checked": props.checked,
        "value": props.value,
        "min": props.min,
        "max": props.max,
        "step": props.step,
        "placeholder": props.placeholder.as_deref(),
        "items_count": props.items.len(),
        "route_value": props.route_value.as_deref(),
        "page": props.page.as_deref(),
        "table": {
            "columns": props.table_columns.len(),
            "rows": props.table_rows,
            "resource_id": props.table_resource_id.as_deref(),
            "page_size": props.page_size,
            "sample_rows": props.table_sample_rows,
        },
    })
}

fn node_snapshot(node: &WidgetNode) -> Value {
    json!({
        "id": &node.id,
        "type": widget_kind_name(&node.kind),
        "key": node.key.as_deref(),
        "class": node.class_name.as_deref(),
        "props": props_snapshot(node),
        "style": &node.style_json,
        "children": node.children.iter().map(node_snapshot).collect::<Vec<_>>(),
    })
}

fn layout_snapshot(layout: Option<&crate::layout::LayoutResult>) -> Value {
    let Some(layout) = layout else {
        return json!({});
    };
    let mut rects = Map::new();
    for (id, rect) in &layout.rects {
        rects.insert(id.clone(), rect_json(*rect));
    }
    Value::Object(rects)
}

fn widget_state_snapshot(state: Option<&WidgetState>) -> Value {
    let Some(state) = state else {
        return json!(null);
    };
    let tables: Map<String, Value> = state
        .tables
        .iter()
        .map(|(id, table)| {
            let sort = table.sort.map(|(col, direction)| {
                json!({
                    "column": col,
                    "direction": match direction {
                        crate::events::SortDirection::Asc => "asc",
                        crate::events::SortDirection::Desc => "desc",
                    },
                })
            });
            (
                id.clone(),
                json!({
                    "columns": table.columns.len(),
                    "rows": table.rows,
                    "resource_id": table.resource_id.as_deref(),
                    "page_size": table.page_size,
                    "scroll_row": table.scroll_row,
                    "scroll_col": table.scroll_col,
                    "selected": table.selected.map(|(row, col)| json!([row, col])),
                    "sort": sort,
                }),
            )
        })
        .collect();
    json!({
        "checked": &state.checked,
        "float_val": &state.float_val,
        "float_range": &state.float_range,
        "text_val": &state.text_val,
        "text_cursor": &state.text_cursor,
        "dropdown_index": &state.dropdown_index,
        "dropdown_items_count": state.dropdown_items.iter().map(|(id, items)| (id.clone(), json!(items.len()))).collect::<Map<_, _>>(),
        "disabled": state.disabled.iter().cloned().collect::<Vec<_>>(),
        "focus_order": &state.focus_order,
        "focused": state.focused.as_deref(),
        "hovered": state.hovered.as_deref(),
        "pressed": state.pressed.as_deref(),
        "open_dropdown": state.open_dropdown.as_deref(),
        "active_tabs": &state.active_tabs,
        "active_pages": &state.active_pages,
        "tables": Value::Object(tables),
    })
}

fn theme_snapshot(theme: &Theme) -> Value {
    json!({
        "background": color_json(theme.background),
        "surface": color_json(theme.surface),
        "surface_alt": color_json(theme.surface_alt),
        "text": color_json(theme.text),
        "muted_text": color_json(theme.muted_text),
        "accent": color_json(theme.accent),
        "border": color_json(theme.border),
        "danger": color_json(theme.danger),
        "warning": color_json(theme.warning),
        "success": color_json(theme.success),
        "focus": color_json(theme.focus),
        "disabled": color_json(theme.disabled),
        "radius": theme.radius,
        "spacing": theme.spacing,
        "font_size": theme.font_size,
    })
}

fn now_epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn set_widget_text_prop(node: &mut WidgetNode, id: &str, prop: &str, value: String) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    match (target.kind.clone(), prop) {
        (WidgetKind::Label | WidgetKind::Button | WidgetKind::Checkbox, "text" | "label") => {
            target.props.text = Some(value);
            true
        }
        (WidgetKind::Panel | WidgetKind::Page, "title") => {
            target.props.text = Some(value);
            true
        }
        (WidgetKind::Tab | WidgetKind::NavItem, "label") => {
            target.props.text = Some(value);
            true
        }
        _ => false,
    }
}

fn set_widget_class_prop(node: &mut WidgetNode, id: &str, value: Option<String>) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    target.class_name = value;
    true
}

fn merge_style_patch(target: &mut Map<String, Value>, patch: &Map<String, Value>) {
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn style_patch_dirty(patch: &Map<String, Value>) -> Dirty {
    let mut dirty = Dirty::Visual;
    for (key, value) in patch {
        if is_layout_style_key(key) {
            return Dirty::Layout;
        }
        if is_text_style_key(key) || pseudo_style_value_changes_text(key, value) {
            dirty = Dirty::Text;
        }
    }
    dirty
}

fn is_layout_style_key(key: &str) -> bool {
    matches!(
        key,
        "display"
            | "flex_direction"
            | "flex"
            | "flex_grow"
            | "flex_shrink"
            | "width"
            | "height"
            | "min_width"
            | "min_height"
            | "max_width"
            | "max_height"
            | "padding"
            | "padding_left"
            | "padding_right"
            | "padding_top"
            | "padding_bottom"
            | "margin"
            | "gap"
    )
}

fn is_text_style_key(key: &str) -> bool {
    matches!(
        key,
        "foreground" | "color" | "font_size" | "font_family" | "font_weight" | "text_align"
    )
}

fn pseudo_style_value_changes_text(key: &str, value: &Value) -> bool {
    if !matches!(key, "hover" | "active" | "focus" | "disabled") {
        return false;
    }
    let Some(map) = value.as_object() else {
        return false;
    };
    map.keys().any(|nested_key| is_text_style_key(nested_key))
}

#[cfg(test)]
mod style_patch_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_style_patch_sets_and_removes_keys() {
        let mut target = json!({
            "background": "surface",
            "border_width": 1,
            "hover": {"background": "accent_mix_20"}
        })
        .as_object()
        .unwrap()
        .clone();
        let patch = json!({
            "background": "danger",
            "border_width": null,
            "hover": {"background": "accent_dark"}
        });

        merge_style_patch(&mut target, patch.as_object().unwrap());

        assert_eq!(target.get("background").unwrap(), "danger");
        assert!(!target.contains_key("border_width"));
        assert_eq!(
            target
                .get("hover")
                .and_then(Value::as_object)
                .and_then(|hover| hover.get("background"))
                .unwrap(),
            "accent_dark"
        );
    }

    #[test]
    fn style_patch_dirty_prefers_layout_over_text_and_visual() {
        assert_eq!(
            style_patch_dirty(json!({"background": "danger"}).as_object().unwrap()),
            Dirty::Visual
        );
        assert_eq!(
            style_patch_dirty(json!({"font_size": 16}).as_object().unwrap()),
            Dirty::Text
        );
        assert_eq!(
            style_patch_dirty(json!({"font_size": 16, "width": 200}).as_object().unwrap()),
            Dirty::Layout
        );
        assert_eq!(
            style_patch_dirty(json!({"hover": {"color": "accent"}}).as_object().unwrap()),
            Dirty::Text
        );
        assert_eq!(
            style_patch_dirty(
                json!({"hover": {"background": "accent"}})
                    .as_object()
                    .unwrap()
            ),
            Dirty::Visual
        );
    }

    #[test]
    fn dirty_names_are_snapshot_safe() {
        assert_eq!(dirty_name(Dirty::Layout), "layout");
        assert_eq!(dirty_name(Dirty::Text), "text");
        assert_eq!(dirty_name(Dirty::Visual), "visual");
        assert_eq!(dirty_name(Dirty::GpuData), "gpu_data");
        assert_eq!(dirty_name(Dirty::Full), "full");
    }

    #[test]
    fn runtime_command_record_serializes_debug_fields() {
        let record = RuntimeCommandRecord {
            seq: 7,
            frame: 3,
            command: "SetStyle".to_string(),
            target: Some("button".to_string()),
            detail: Some("patch_bytes=42".to_string()),
            dirty: Some(Dirty::Visual),
            outcome: "applied".to_string(),
            requested_redraw: true,
        };

        let value = record.json_value();

        assert_eq!(value["seq"], 7);
        assert_eq!(value["frame"], 3);
        assert_eq!(value["command"], "SetStyle");
        assert_eq!(value["target"], "button");
        assert_eq!(value["detail"], "patch_bytes=42");
        assert_eq!(value["dirty"], "visual");
        assert_eq!(value["outcome"], "applied");
        assert_eq!(value["requested_redraw"], true);
    }

    #[test]
    fn parse_widget_children_json_parses_node_list() {
        let children = parse_widget_children_json(
            r#"[{"id":"label","type":"label","props":{"text":"hello"}}]"#,
        )
        .unwrap();

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, "label");
        assert_eq!(children[0].kind, WidgetKind::Label);
        assert_eq!(children[0].props.text.as_deref(), Some("hello"));
    }

    #[test]
    fn replace_widget_children_swaps_target_subtree() {
        let mut root = document::parse_widget_node(&json!({
            "id": "window",
            "type": "window",
            "props": {},
            "children": [{
                "id": "panel",
                "type": "panel",
                "props": {"title": "Panel"},
                "children": [{"id": "old", "type": "label", "props": {"text": "old"}}]
            }]
        }))
        .unwrap();
        let children =
            parse_widget_children_json(r#"[{"id":"new","type":"label","props":{"text":"new"}}]"#)
                .unwrap();

        assert!(replace_widget_children(&mut root, "panel", children));

        let panel = find_widget_mut(&mut root, "panel").unwrap();
        assert_eq!(panel.children.len(), 1);
        assert_eq!(panel.children[0].id, "new");
        assert_eq!(panel.children[0].props.text.as_deref(), Some("new"));
    }

    #[test]
    fn replace_widget_node_swaps_target_node() {
        let mut root = document::parse_widget_node(&json!({
            "id": "window",
            "type": "window",
            "props": {},
            "children": [{
                "id": "old",
                "type": "label",
                "props": {"text": "old"}
            }]
        }))
        .unwrap();
        let replacement =
            parse_widget_node_json(r#"{"id":"new","type":"button","props":{"text":"new button"}}"#)
                .unwrap();

        assert!(replace_widget_node(&mut root, "old", replacement));

        assert!(find_widget_mut(&mut root, "old").is_none());
        let button = find_widget_mut(&mut root, "new").unwrap();
        assert_eq!(button.kind, WidgetKind::Button);
        assert_eq!(button.props.text.as_deref(), Some("new button"));
    }

    #[test]
    fn find_first_widget_kind_id_finds_nested_scatter() {
        let root = document::parse_widget_node(&json!({
            "id": "window",
            "type": "window",
            "props": {},
            "children": [{
                "id": "panel",
                "type": "panel",
                "props": {},
                "children": [{
                    "id": "scatter",
                    "type": "scatter_3d",
                    "props": {"x": "x", "y": "y", "z": "z"}
                }]
            }]
        }))
        .unwrap();

        assert_eq!(
            find_first_widget_kind_id(&root, &WidgetKind::Scatter3D),
            Some("scatter")
        );
        assert_eq!(find_first_widget_kind_id(&root, &WidgetKind::Slider), None);
    }

    #[test]
    fn decode_scatter_points_reuses_output_vec() {
        let mut out = Vec::with_capacity(8);
        let mut bytes = Vec::new();
        for value in [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        decode_scatter_points_bytes_into_colormap(&bytes, &mut out, "viridis").unwrap();

        assert_eq!(out.len(), 2);
        assert!(out.capacity() >= 8);
        assert_eq!(out[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(out[1].position, [4.0, 5.0, 6.0]);
    }

    #[test]
    fn parse_table_update_json_extracts_table_props() {
        let props = parse_table_update_json(
            "table",
            r#"{"frame":{"columns":["x","y"],"dtypes":["f32","f32"],"rows":2},"resource_id":"table:resource","page_size":25,"sample_rows":2,"cells":[["1","2"],["3","4"]]}"#,
        )
        .unwrap();

        assert_eq!(props.table_columns, vec!["x", "y"]);
        assert_eq!(props.table_dtypes, vec!["f32", "f32"]);
        assert_eq!(props.table_rows, Some(2));
        assert_eq!(props.table_resource_id.as_deref(), Some("table:resource"));
        assert_eq!(props.table_sample_rows, Some(2));
        assert_eq!(props.page_size, Some(25));
        assert_eq!(props.table_cells.len(), 2);
    }

    #[test]
    fn set_widget_text_prop_updates_retained_node_text() {
        let mut root = document::parse_widget_node(&json!({
            "id": "window",
            "type": "window",
            "props": {},
            "children": [{
                "id": "status",
                "type": "label",
                "props": {"text": "old"}
            }]
        }))
        .unwrap();

        assert!(set_widget_text_prop(
            &mut root,
            "status",
            "text",
            "new".to_string()
        ));
        let label = find_widget_mut(&mut root, "status").unwrap();
        assert_eq!(label.props.text.as_deref(), Some("new"));
    }

    #[test]
    fn set_widget_class_prop_updates_retained_metadata() {
        let mut root = document::parse_widget_node(&json!({
            "id": "window",
            "type": "window",
            "props": {},
            "children": [{
                "id": "status",
                "class": "old",
                "type": "label",
                "props": {"text": "ready"}
            }]
        }))
        .unwrap();

        assert!(set_widget_class_prop(
            &mut root,
            "status",
            Some("new".to_string())
        ));
        let label = find_widget_mut(&mut root, "status").unwrap();
        assert_eq!(label.class_name.as_deref(), Some("new"));

        assert!(set_widget_class_prop(&mut root, "status", None));
        let label = find_widget_mut(&mut root, "status").unwrap();
        assert_eq!(label.class_name, None);
    }
}

// ---------------------------------------------------------------------------
// GPU state
// ---------------------------------------------------------------------------

struct WgpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    _depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    theme: Theme,
    scale_factor: f32,
    scatter: Option<ScatterWidget>,
    scatter_widget_id: Option<String>,
    scatter_decode_scratch: Vec<PointInstance>,
    primitives: Option<PrimitivesRenderer>,
    widget_tree: Option<WidgetNode>,
    widget_kinds: HashMap<String, WidgetKind>,
    caret_positions: HashMap<String, f32>,
    resources: ResourceRegistry,
    /// Mutable per-widget interactive state (checkbox, slider, hover, press).
    widget_state: Option<WidgetState>,
    /// Layout saved after each `apply_layout` call for hit testing.
    current_layout: Option<crate::layout::LayoutResult>,
    /// Text renderer (Label, Button labels).
    text: Option<TextRendererDg>,
    scatter_metrics: ScatterMetrics,
}

#[derive(Debug, Clone, Default)]
struct ScatterMetrics {
    updates: u64,
    last_point_count: usize,
    last_payload_bytes: usize,
    last_pack_ms: f64,
    last_queue_latency_ms: f64,
    last_decode_ms: f64,
    last_upload_ms: f64,
    last_total_native_ms: f64,
}

impl WgpuState {
    async fn new(window: Arc<Window>, spec: AppSpec) -> Result<(Self, f64), DragonError> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);
        let scale_factor = window.scale_factor() as f32;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(|e| DragonError::GpuInit(format!("surface: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| DragonError::GpuInit(format!("adapter: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("dragongui"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: Default::default(),
            })
            .await
            .map_err(|e| DragonError::GpuInit(format!("device: {e}")))?;

        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| DragonError::GpuInit("unsupported surface format".into()))?;
        surface.configure(&device, &config);

        let (depth_texture, depth_view) = create_depth_texture(&device, width, height);
        let theme = spec.theme.unwrap_or_else(Theme::dark);

        let (scatter, upload_ms) = if let Some(scatter_spec) = spec.scatter {
            let mut s = ScatterWidget::new(&device, config.format, width, height);
            let t0 = Instant::now();
            let pts = if let Some(b64) = scatter_spec.data_b64 {
                decode_scatter_points(&b64, &scatter_spec.colormap)?
            } else {
                gen_demo_points_with_colormap(&scatter_spec.colormap)
            };
            s.set_points(&device, &queue, &pts);
            s.update_camera(&queue);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            (Some(s), ms)
        } else {
            (None, 0.0)
        };

        let primitives = spec
            .widget_tree
            .as_ref()
            .map(|_| PrimitivesRenderer::new(&device, &queue, config.format, width, height));

        let text = spec
            .widget_tree
            .as_ref()
            .map(|_| TextRendererDg::new(&device, &queue, config.format));

        let mut resources = ResourceRegistry::default();
        if let Some(tree) = &spec.widget_tree {
            resources.sync_from_tree(tree);
        }
        let widget_state = spec.widget_tree.as_ref().map(WidgetState::from_tree);
        let mut widget_kinds = HashMap::new();
        if let Some(tree) = &spec.widget_tree {
            collect_widget_kinds(tree, &mut widget_kinds);
        }

        let scatter_widget_id = spec
            .widget_tree
            .as_ref()
            .and_then(|tree| find_first_widget_kind_id(tree, &WidgetKind::Scatter3D))
            .map(str::to_string);

        let mut state = Self {
            surface,
            device,
            queue,
            config,
            _depth_texture: depth_texture,
            depth_view,
            theme,
            scale_factor,
            scatter,
            scatter_widget_id,
            scatter_decode_scratch: Vec::new(),
            primitives,
            widget_tree: spec.widget_tree,
            widget_kinds,
            caret_positions: HashMap::new(),
            resources,
            widget_state,
            current_layout: None,
            text,
            scatter_metrics: ScatterMetrics::default(),
        };

        state.apply_layout();

        Ok((state, upload_ms))
    }

    /// Recompute layout and push scatter viewport + primitives + text to GPU.
    fn apply_layout(&mut self) {
        // Destructure to get separate borrows of each field.
        let WgpuState {
            widget_tree,
            current_layout,
            widget_state,
            resources,
            primitives,
            text,
            scatter,
            caret_positions,
            device,
            queue,
            config,
            theme,
            scale_factor,
            ..
        } = self;

        let tree = match widget_tree.as_ref() {
            Some(t) => t,
            None => return,
        };

        let w = config.width as f32;
        let h = config.height as f32;
        let layout = compute_layout(tree, w, h, *scale_factor, theme, widget_state.as_ref());

        if let Some(s) = scatter {
            if let Some(scatter_id) = find_visible_scatter_id(tree, &layout) {
                if let Some(r) = layout.rects.get(scatter_id) {
                    s.set_layout_rect(r.x, r.y, r.w, r.h, queue);
                }
            } else {
                s.set_layout_rect(0.0, 0.0, 0.0, 0.0, queue);
            }
        }

        if let Some(state) = widget_state.as_mut() {
            if state
                .focused
                .as_ref()
                .is_some_and(|id| !layout.rects.contains_key(id))
            {
                state.focused = None;
            }
        }

        let new_caret_positions =
            if let (Some(t), Some(state)) = (text.as_mut(), widget_state.as_ref()) {
                t.update_screen(queue, config.width, config.height);
                t.rebuild(tree, &layout, theme, *scale_factor, state, resources)
            } else {
                HashMap::new()
            };
        *caret_positions = new_caret_positions;

        if let (Some(prims), Some(state)) = (primitives.as_mut(), widget_state.as_ref()) {
            prims.update_screen_size(queue, config.width, config.height);
            prims.rebuild(
                device,
                queue,
                tree,
                &layout,
                theme,
                *scale_factor,
                state,
                caret_positions,
            );
        }

        *current_layout = Some(layout);
    }

    fn rebuild_primitives(&mut self) {
        let WgpuState {
            widget_tree,
            current_layout,
            widget_state,
            primitives,
            device,
            queue,
            theme,
            scale_factor,
            caret_positions,
            ..
        } = self;

        if let (Some(tree), Some(layout), Some(state), Some(prims)) = (
            widget_tree.as_ref(),
            current_layout.as_ref(),
            widget_state.as_ref(),
            primitives.as_mut(),
        ) {
            prims.rebuild(
                device,
                queue,
                tree,
                layout,
                theme,
                *scale_factor,
                state,
                caret_positions,
            );
        }
    }

    fn rebuild_text(&mut self) {
        let WgpuState {
            widget_tree,
            current_layout,
            widget_state,
            text,
            theme,
            scale_factor,
            caret_positions,
            resources,
            ..
        } = self;

        if let (Some(tree), Some(layout), Some(state), Some(t)) = (
            widget_tree.as_ref(),
            current_layout.as_ref(),
            widget_state.as_ref(),
            text.as_mut(),
        ) {
            *caret_positions = t.rebuild(tree, layout, theme, *scale_factor, state, resources);
        } else {
            caret_positions.clear();
        }
    }

    /// Rebuild state-dependent primitive and text buffers without recomputing layout.
    fn rebuild_visuals(&mut self) {
        self.rebuild_text();
        self.rebuild_primitives();
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        let (dt, dv) = create_depth_texture(&self.device, width, height);
        self._depth_texture = dt;
        self.depth_view = dv;
        self.apply_layout();
    }

    fn set_scale_factor(&mut self, sf: f64, new_inner_size: PhysicalSize<u32>) {
        self.scale_factor = sf as f32;
        self.resize(new_inner_size.width, new_inner_size.height);
    }

    fn scatter_contains(&self, pos: [f32; 2]) -> bool {
        self.scatter
            .as_ref()
            .map(|s| s.contains_point(pos[0], pos[1]))
            .unwrap_or(false)
    }

    /// Hit test interactive UI widgets at physical pixel position `pos`.
    fn hit_test_ui(&self, pos: [f32; 2]) -> Option<(String, WidgetKind)> {
        let (tree, layout) = match (self.widget_tree.as_ref(), self.current_layout.as_ref()) {
            (Some(t), Some(l)) => (t, l),
            _ => return None,
        };
        let state = self.widget_state.as_ref()?;
        hit_test(tree, layout, pos).filter(|(id, _)| !state.is_disabled(id))
    }

    /// Look up the kind of widget with `id` in the widget tree.
    fn widget_kind(&self, id: &str) -> Option<WidgetKind> {
        self.widget_kinds.get(id).cloned()
    }

    fn has_widget(&self, id: &str) -> bool {
        self.widget_kinds.contains_key(id)
    }

    fn apply_set_prop(&mut self, id: &str, prop: &str, value: CommandValue) -> Option<Dirty> {
        let kind = self.widget_kind(id)?;
        if prop == "class" {
            let class_name = match value {
                CommandValue::Text(text) => Some(text),
                CommandValue::None => None,
                other => {
                    eprintln!(
                        "DragonGUI: ignoring unsupported live class metadata value for widget {id:?}: {other:?}"
                    );
                    return None;
                }
            };
            if let Some(tree) = self.widget_tree.as_mut() {
                if set_widget_class_prop(tree, id, class_name) {
                    return Some(Dirty::Visual);
                }
            }
            return None;
        }
        if let CommandValue::Text(text) = &value {
            if matches!(
                kind,
                WidgetKind::Label
                    | WidgetKind::Button
                    | WidgetKind::Panel
                    | WidgetKind::Checkbox
                    | WidgetKind::Page
                    | WidgetKind::Tab
                    | WidgetKind::NavItem
            ) {
                if let Some(tree) = self.widget_tree.as_mut() {
                    if set_widget_text_prop(tree, id, prop, text.clone()) {
                        return Some(Dirty::Text);
                    }
                }
            }
        }
        let state = self.widget_state.as_mut()?;
        match (kind, prop, value) {
            (WidgetKind::Checkbox, "checked", CommandValue::Bool(v)) => {
                state.set_checked(id, v)?;
                Some(Dirty::Visual)
            }
            (WidgetKind::Slider, "value", CommandValue::Float(v)) => {
                state.try_set_float(id, v)?;
                Some(Dirty::Visual)
            }
            (WidgetKind::Dropdown, "value", CommandValue::Text(v)) => {
                state.set_dropdown_value(id, &v)?;
                Some(Dirty::Text)
            }
            (WidgetKind::TextInput, "value", CommandValue::Text(v)) => {
                state.set_text_value(id, v)?;
                Some(Dirty::Text)
            }
            (kind, prop, _) => {
                eprintln!(
                    "DragonGUI: ignoring unsupported live SetProp for widget {id:?} ({kind:?}).{prop}"
                );
                None
            }
        }
    }

    fn apply_set_style_patch(
        &mut self,
        id: &str,
        patch_json: &str,
    ) -> Result<Option<Dirty>, DragonError> {
        if !self.has_widget(id) {
            return Ok(None);
        }
        let patch: Value = serde_json::from_str(patch_json).map_err(|e| {
            DragonError::Runtime(format!("invalid live style patch for widget {id:?}: {e}"))
        })?;
        let Some(patch) = patch.as_object() else {
            return Err(DragonError::Runtime(format!(
                "live style patch for widget {id:?} must be a JSON object"
            )));
        };
        if patch.is_empty() {
            return Ok(None);
        }
        let Some(tree) = self.widget_tree.as_mut() else {
            return Ok(None);
        };
        let Some(node) = find_widget_mut(tree, id) else {
            return Ok(None);
        };
        merge_style_patch(&mut node.style_json, patch);
        node.style = NodeStyle::from_json(Some(&Value::Object(node.style_json.clone())));
        Ok(Some(style_patch_dirty(patch)))
    }

    fn rebuild_retained_maps(&mut self) {
        let mut widget_kinds = HashMap::new();
        let scatter_id = self
            .widget_tree
            .as_ref()
            .and_then(|tree| find_first_widget_kind_id(tree, &WidgetKind::Scatter3D))
            .map(str::to_string);
        let widget_state = self.widget_tree.as_ref().map(|tree| {
            collect_widget_kinds(tree, &mut widget_kinds);
            self.resources.sync_from_tree(tree);
            WidgetState::from_tree(tree)
        });
        if self.widget_tree.is_none() {
            self.resources = ResourceRegistry::default();
        }
        match (scatter_id.as_deref(), self.scatter_widget_id.as_deref()) {
            (None, _) => {
                self.scatter = None;
                self.scatter_widget_id = None;
                self.scatter_decode_scratch.clear();
                self.scatter_metrics = ScatterMetrics::default();
            }
            (Some(next_id), Some(current_id)) if next_id == current_id => {}
            (Some(_), _) => {
                self.scatter = Some(ScatterWidget::new(
                    &self.device,
                    self.config.format,
                    self.config.width.max(1),
                    self.config.height.max(1),
                ));
                self.scatter_widget_id = scatter_id;
                self.scatter_decode_scratch.clear();
                self.scatter_metrics = ScatterMetrics::default();
            }
        }
        self.widget_kinds = widget_kinds;
        self.widget_state = widget_state;
    }

    fn apply_replace_children(
        &mut self,
        id: &str,
        children_json: &str,
    ) -> Result<bool, DragonError> {
        if !self.has_widget(id) {
            return Ok(false);
        }
        let children = parse_widget_children_json(children_json)?;
        let Some(tree) = self.widget_tree.as_mut() else {
            return Ok(false);
        };
        if !replace_widget_children(tree, id, children) {
            return Ok(false);
        }

        self.rebuild_retained_maps();
        Ok(true)
    }

    fn apply_replace_node(&mut self, id: &str, node_json: &str) -> Result<bool, DragonError> {
        if !self.has_widget(id) {
            return Ok(false);
        }
        let replacement = parse_widget_node_json(node_json)?;
        let Some(tree) = self.widget_tree.as_mut() else {
            return Ok(false);
        };
        if !replace_widget_node(tree, id, replacement) {
            return Ok(false);
        }

        self.rebuild_retained_maps();
        Ok(true)
    }

    fn apply_set_table_data(&mut self, id: &str, table_json: &str) -> Result<bool, DragonError> {
        if self.widget_kind(id) != Some(WidgetKind::DataFrameTable) {
            return Ok(false);
        }
        let props = parse_table_update_json(id, table_json)?;
        let Some(tree) = self.widget_tree.as_mut() else {
            return Ok(false);
        };
        let Some(node) = find_widget_mut(tree, id) else {
            return Ok(false);
        };
        node.props.table_columns = props.table_columns;
        node.props.table_dtypes = props.table_dtypes;
        node.props.table_rows = props.table_rows;
        node.props.table_resource_id = props.table_resource_id;
        node.props.table_sample_rows = props.table_sample_rows;
        node.props.page_size = props.page_size;
        node.props.table_cells = props.table_cells;

        self.rebuild_retained_maps();
        Ok(true)
    }

    fn apply_set_table_data_columns(
        &mut self,
        id: &str,
        table_json: &str,
        columns: Vec<TableColumnPacket>,
    ) -> Result<bool, DragonError> {
        if self.widget_kind(id) != Some(WidgetKind::DataFrameTable) {
            return Ok(false);
        }
        let props = parse_table_update_json(id, table_json)?;
        let Some(resource_id) = props.table_resource_id.clone() else {
            return Err(DragonError::Runtime(
                "table column update requires a resource_id".to_string(),
            ));
        };
        let Some(tree) = self.widget_tree.as_mut() else {
            return Ok(false);
        };
        let Some(node) = find_widget_mut(tree, id) else {
            return Ok(false);
        };
        node.props.table_columns = props.table_columns.clone();
        node.props.table_dtypes = props.table_dtypes.clone();
        node.props.table_rows = props.table_rows;
        node.props.table_resource_id = props.table_resource_id.clone();
        node.props.table_sample_rows = props.table_sample_rows;
        node.props.page_size = props.page_size;
        node.props.table_cells = props.table_cells.clone();

        self.resources
            .update_table_columns(&resource_id, &props, columns);
        self.rebuild_retained_maps();
        Ok(true)
    }

    fn apply_set_buffer_resource(
        &mut self,
        id: &str,
        kind: &str,
        bytes: Vec<u8>,
        owner_id: Option<String>,
    ) -> bool {
        self.resources
            .update_buffer(id.to_string(), kind.to_string(), bytes, owner_id);
        true
    }

    fn apply_release_resource(&mut self, id: &str) -> bool {
        self.resources.release(id)
    }

    fn set_scatter_points_packed(
        &mut self,
        id: &str,
        xyz: Vec<u8>,
        telemetry: Option<ScatterTelemetry>,
        colormap: String,
    ) -> Result<bool, DragonError> {
        if self.widget_kind(id) != Some(WidgetKind::Scatter3D) {
            return Ok(false);
        }
        let total_t0 = Instant::now();
        let queue_latency_ms = telemetry
            .as_ref()
            .map(|t| (now_epoch_ms() - t.enqueue_epoch_ms).max(0.0))
            .unwrap_or(0.0);
        if self.scatter.is_none() {
            return Ok(false);
        }
        let decode_t0 = Instant::now();
        decode_scatter_points_bytes_into_colormap(
            &xyz,
            &mut self.scatter_decode_scratch,
            &colormap,
        )?;
        let decode_ms = decode_t0.elapsed().as_secs_f64() * 1000.0;
        let Some(scatter) = self.scatter.as_mut() else {
            return Ok(false);
        };
        let upload_t0 = Instant::now();
        scatter.set_points(&self.device, &self.queue, &self.scatter_decode_scratch);
        let upload_ms = upload_t0.elapsed().as_secs_f64() * 1000.0;
        let point_count = self.scatter_decode_scratch.len();
        let payload_bytes = xyz.len();
        let pack_ms = telemetry.as_ref().map(|t| t.pack_ms).unwrap_or(0.0);
        let reported_point_count = telemetry
            .as_ref()
            .map(|t| t.point_count)
            .unwrap_or(point_count);
        let reported_payload_bytes = telemetry
            .as_ref()
            .map(|t| t.payload_bytes)
            .unwrap_or(payload_bytes);
        self.scatter_metrics = ScatterMetrics {
            updates: self.scatter_metrics.updates + 1,
            last_point_count: reported_point_count,
            last_payload_bytes: reported_payload_bytes,
            last_pack_ms: pack_ms,
            last_queue_latency_ms: queue_latency_ms,
            last_decode_ms: decode_ms,
            last_upload_ms: upload_ms,
            last_total_native_ms: total_t0.elapsed().as_secs_f64() * 1000.0,
        };
        Ok(true)
    }

    fn rebuild_for_dirty(&mut self, dirty: Dirty) {
        match dirty {
            Dirty::Layout | Dirty::Full => self.apply_layout(),
            Dirty::Text => self.rebuild_visuals(),
            Dirty::Visual => self.rebuild_primitives(),
            Dirty::GpuData => {}
        }
    }

    fn debug_snapshot_value(&self) -> Value {
        json!({
            "window": {
                "width": self.config.width,
                "height": self.config.height,
                "scale_factor": self.scale_factor,
            },
            "theme": theme_snapshot(&self.theme),
            "tree": self.widget_tree.as_ref().map(node_snapshot),
            "layout": layout_snapshot(self.current_layout.as_ref()),
            "state": widget_state_snapshot(self.widget_state.as_ref()),
            "renderer": {
                "surface_format": format!("{:?}", self.config.format),
                "has_primitives": self.primitives.is_some(),
                "has_text": self.text.is_some(),
                "has_scatter": self.scatter.is_some(),
                "scatter_widget_id": self.scatter_widget_id.as_deref(),
                "widget_count": self.widget_kinds.len(),
                "caret_positions": &self.caret_positions,
            },
            "resources": {
                "scatter": {
                    "present": self.scatter.is_some(),
                    "updates": self.scatter_metrics.updates,
                    "last_point_count": self.scatter_metrics.last_point_count,
                    "last_payload_bytes": self.scatter_metrics.last_payload_bytes,
                    "last_pack_ms": self.scatter_metrics.last_pack_ms,
                    "last_queue_latency_ms": self.scatter_metrics.last_queue_latency_ms,
                    "last_decode_ms": self.scatter_metrics.last_decode_ms,
                    "last_upload_ms": self.scatter_metrics.last_upload_ms,
                    "last_total_native_ms": self.scatter_metrics.last_total_native_ms,
                },
                "registry": self.resources.snapshot(),
                "tables": {
                    "widgets": self.widget_state.as_ref().map(|state| state.tables.len()).unwrap_or(0),
                    "resources": self.resources.table_count(),
                },
                "buffers": {
                    "resources": self.resources.buffer_count(),
                },
            },
        })
    }

    /// Build a `SliderDrag` for widget `id` from the current layout and state.
    fn create_slider_drag(&self, id: &str) -> Option<SliderDrag> {
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        let rect = layout.rects.get(id)?;
        let (min, max) = state.float_range.get(id).copied()?;
        Some(SliderDrag::new(
            id.to_string(),
            rect,
            min,
            max,
            self.scale_factor,
        ))
    }

    fn dropdown_option_at(&self, pos: [f32; 2]) -> Option<(String, usize)> {
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        let id = state.open_dropdown.as_ref()?;
        let rect = layout.rects.get(id)?;
        let items = state.dropdown_items.get(id)?;
        let row_h = self.theme.control_height() * self.scale_factor;
        if pos[0] < rect.x || pos[0] >= rect.x + rect.w || pos[1] < rect.y + rect.h {
            return None;
        }
        let idx = ((pos[1] - rect.y - rect.h) / row_h).floor() as usize;
        if idx < items.len() {
            Some((id.clone(), idx))
        } else {
            None
        }
    }

    fn table_at(&self, pos: [f32; 2]) -> Option<String> {
        self.hit_test_ui(pos)
            .and_then(|(id, kind)| (kind == WidgetKind::DataFrameTable).then_some(id))
    }

    fn table_hit(&self, pos: [f32; 2]) -> Option<(String, TableHit)> {
        let (id, kind) = self.hit_test_ui(pos)?;
        if kind != WidgetKind::DataFrameTable {
            return None;
        }
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        let table_state = state.table(&id)?;
        let rect = layout.rects.get(&id)?;
        let metrics = table::metrics(&self.theme, self.scale_factor);
        table::hit(table_state, rect, metrics, pos).map(|hit| (id, hit))
    }

    fn table_visible_counts(&self, id: &str) -> Option<(usize, usize)> {
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        let table_state = state.table(id)?;
        let rect = layout.rects.get(id)?;
        let metrics = table::metrics(&self.theme, self.scale_factor);
        let visible = table::visible(table_state, rect, metrics);
        Some((visible.row_count, visible.col_count))
    }

    fn focus_widget(&mut self, id: Option<String>) {
        if let Some(ws) = &mut self.widget_state {
            ws.focus_widget(id);
        }
        self.rebuild_visuals();
    }

    fn focused_kind(&self) -> Option<(String, WidgetKind)> {
        let id = self.widget_state.as_ref()?.focused.clone()?;
        let kind = self.widget_kind(&id)?;
        Some((id, kind))
    }

    fn focused_text_input_rect(&self) -> Option<crate::layout::Rect> {
        let (id, kind) = self.focused_kind()?;
        if kind != WidgetKind::TextInput {
            return None;
        }
        self.current_layout.as_ref()?.rects.get(&id).copied()
    }

    fn render(&mut self) -> Result<(), DragonError> {
        // Prepare text glyph uploads before acquiring the render pass (avoids
        // borrow conflicts with depth_view which is borrowed by the pass).
        {
            let WgpuState {
                text,
                device,
                queue,
                ..
            } = &mut *self;
            if let Some(t) = text.as_mut() {
                t.prepare(device, queue);
            }
        }

        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                self.surface.configure(&self.device, &self.config);
                t
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
        };

        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dragongui-frame"),
            });

        {
            let bg = self.theme.background;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dragongui-main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0] as f64,
                            g: bg[1] as f64,
                            b: bg[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // 1. Primitive rects (panels, control shells) — no depth write.
            if let Some(prims) = &self.primitives {
                prims.render(&mut pass);
            }

            // 2. Scatter — uses depth buffer, restricted to its viewport rect.
            if let Some(s) = &self.scatter {
                s.render(&mut pass);
            }

            // 3. Text overlay — no depth write, always on top.
            pass.set_viewport(
                0.0,
                0.0,
                self.config.width as f32,
                self.config.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
            // TextRenderer::render is &self so no borrow conflict with depth_view.
            if let Some(t) = &self.text {
                t.render(&mut pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        texture.present();

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// winit ApplicationHandler
// ---------------------------------------------------------------------------

const SLIDER_CALLBACK_INTERVAL: Duration = Duration::from_millis(33);
const SLIDER_CHANGE_EPSILON: f32 = 0.000_001;
const COMMAND_HISTORY_LIMIT: usize = 64;

struct SliderChangeDispatch {
    widget_id: String,
    value: f32,
    at: Instant,
}

#[derive(Debug, Clone)]
struct RuntimeCommandRecord {
    seq: u64,
    frame: u32,
    command: String,
    target: Option<String>,
    detail: Option<String>,
    dirty: Option<Dirty>,
    outcome: String,
    requested_redraw: bool,
}

impl RuntimeCommandRecord {
    fn json_value(&self) -> Value {
        json!({
            "seq": self.seq,
            "frame": self.frame,
            "command": self.command,
            "target": self.target,
            "detail": self.detail,
            "dirty": self.dirty.map(dirty_name),
            "outcome": self.outcome,
            "requested_redraw": self.requested_redraw,
        })
    }
}

fn dirty_name(dirty: Dirty) -> &'static str {
    match dirty {
        Dirty::Layout => "layout",
        Dirty::Text => "text",
        Dirty::Visual => "visual",
        Dirty::GpuData => "gpu_data",
        Dirty::Full => "full",
    }
}

struct DragonApp {
    spec: Option<AppSpec>,
    command_bridge: Option<Arc<CommandBridge>>,
    python_runtime: Option<Py<PyAny>>,
    window: Option<Arc<Window>>,
    gpu: Option<WgpuState>,
    error: Option<DragonError>,
    smoke_frames: Option<u32>,
    frames_rendered: u32,
    upload_ms: f64,
    frame_ms_total: f64,
    last_mouse_pos: Option<[f32; 2]>,
    orbit_active: bool,
    pan_active: bool,
    /// Button on_click callbacks (moved out of AppSpec in `resumed`).
    click_cbs: HashMap<String, Box<dyn Fn() + Send>>,
    /// Checkbox / Slider on_change callbacks.
    change_cbs: HashMap<String, Box<dyn Fn(ChangeValue) + Send>>,
    /// Active slider drag session (pointer-down on a Slider widget).
    slider_drag: Option<SliderDrag>,
    /// Last slider value sent to Python during drag throttling.
    last_slider_emit: Option<SliderChangeDispatch>,
    /// Most recent slider value waiting for a throttled callback slot.
    pending_slider_emit: Option<(String, f32)>,
    /// Id of the UI widget that received the current pointer-down.
    pressed_id: Option<String>,
    /// Currently active keyboard modifiers.
    modifiers: ModifiersState,
    command_seq: u64,
    command_history: VecDeque<RuntimeCommandRecord>,
}

impl DragonApp {
    fn new(mut spec: AppSpec, smoke_frames: Option<u32>) -> Self {
        let command_bridge = spec.command_bridge.take();
        let python_runtime = spec.python_runtime.take();
        Self {
            spec: Some(spec),
            command_bridge,
            python_runtime,
            window: None,
            gpu: None,
            error: None,
            smoke_frames,
            frames_rendered: 0,
            upload_ms: 0.0,
            frame_ms_total: 0.0,
            last_mouse_pos: None,
            orbit_active: false,
            pan_active: false,
            click_cbs: HashMap::new(),
            change_cbs: HashMap::new(),
            slider_drag: None,
            last_slider_emit: None,
            pending_slider_emit: None,
            pressed_id: None,
            modifiers: ModifiersState::empty(),
            command_seq: 0,
            command_history: VecDeque::with_capacity(COMMAND_HISTORY_LIMIT),
        }
    }

    fn take_error(&mut self) -> Option<DragonError> {
        self.error.take()
    }

    fn run_result(&self) -> RunResult {
        let frame_ms = if self.frames_rendered > 0 {
            self.frame_ms_total / self.frames_rendered as f64
        } else {
            0.0
        };
        RunResult {
            upload_ms: self.upload_ms,
            frame_ms,
            debug_snapshot: self.debug_snapshot_json(),
        }
    }

    fn debug_snapshot_json(&self) -> String {
        serde_json::to_string(&self.debug_snapshot_value()).unwrap_or_else(|e| {
            format!(r#"{{"schema":1,"error":"failed to serialize debug snapshot: {e}"}}"#)
        })
    }

    fn record_runtime_command(
        &mut self,
        command: &str,
        target: Option<String>,
        detail: Option<String>,
        dirty: Option<Dirty>,
        outcome: &str,
        requested_redraw: bool,
    ) -> bool {
        self.command_seq += 1;
        if self.command_history.len() == COMMAND_HISTORY_LIMIT {
            self.command_history.pop_front();
        }
        self.command_history.push_back(RuntimeCommandRecord {
            seq: self.command_seq,
            frame: self.frames_rendered,
            command: command.to_string(),
            target,
            detail,
            dirty,
            outcome: outcome.to_string(),
            requested_redraw,
        });
        requested_redraw
    }

    fn command_history_snapshot(&self) -> Value {
        let recent = self
            .command_history
            .iter()
            .map(RuntimeCommandRecord::json_value)
            .collect::<Vec<_>>();
        json!({
            "limit": COMMAND_HISTORY_LIMIT,
            "count": recent.len(),
            "recent": recent,
        })
    }

    fn dirty_history_snapshot(&self) -> Value {
        let recent = self
            .command_history
            .iter()
            .filter(|record| record.dirty.is_some())
            .map(RuntimeCommandRecord::json_value)
            .collect::<Vec<_>>();
        json!({
            "current": null,
            "recent": recent,
            "note": "DragonGUI applies coarse dirty flags immediately; this history records the recent command reasons that triggered them."
        })
    }

    fn debug_snapshot_value(&self) -> Value {
        let frame_ms = if self.frames_rendered > 0 {
            self.frame_ms_total / self.frames_rendered as f64
        } else {
            0.0
        };
        let queue_depth = self
            .command_bridge
            .as_ref()
            .map(|bridge| bridge.len())
            .unwrap_or(0);
        json!({
            "schema": 1,
            "runtime": {
                "window_open": self.window.is_some(),
                "gpu_ready": self.gpu.is_some(),
                "frames_rendered": self.frames_rendered,
                "upload_ms": self.upload_ms,
                "frame_ms": frame_ms,
                "command_queue_depth": queue_depth,
                "smoke_frames": self.smoke_frames,
                "orbit_active": self.orbit_active,
                "pan_active": self.pan_active,
                "pressed_id": self.pressed_id.as_deref(),
                "last_mouse_pos": self.last_mouse_pos,
                "pending_slider_emit": self.pending_slider_emit.as_ref().map(|(id, value)| json!({
                    "id": id,
                    "value": value,
                })),
                "dirty": self.dirty_history_snapshot(),
                "commands": self.command_history_snapshot(),
            },
            "gpu": self.gpu.as_ref().map(WgpuState::debug_snapshot_value),
        })
    }

    fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn drain_runtime_commands(&mut self) {
        if self.window.is_none() || self.gpu.is_none() {
            return;
        }
        let Some(bridge) = self.command_bridge.as_ref().cloned() else {
            return;
        };

        let mut request_redraw = false;
        let mut commands = Vec::new();
        let mut batches = 0_usize;
        loop {
            commands.clear();
            bridge.drain_into(&mut commands);
            if commands.is_empty() {
                break;
            }
            batches += 1;
            for command in commands.drain(..) {
                request_redraw |= self.apply_runtime_command(command);
            }
            if batches >= MAX_COMMAND_DRAIN_BATCHES {
                if !bridge.is_empty() {
                    eprintln!(
                        "DragonGUI: command drain reached batch limit; deferring remaining commands"
                    );
                    bridge.wake();
                }
                break;
            }
        }

        if request_redraw {
            self.request_redraw();
        }
    }

    fn apply_runtime_command(&mut self, command: Command) -> bool {
        match command {
            Command::DrainPythonTasks => {
                let mut outcome = "applied";
                if let Some(handle) = &self.python_runtime {
                    Python::with_gil(|py| {
                        if let Err(err) = handle.call_method0(py, "_drain_python_tasks") {
                            err.print(py);
                            outcome = "python_error";
                        }
                    });
                } else {
                    outcome = "no_python_runtime";
                }
                self.record_runtime_command("DrainPythonTasks", None, None, None, outcome, false)
            }
            Command::Invalidate { id, dirty } => {
                let (outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "Invalidate",
                            Some(id),
                            None,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    if !gpu.has_widget(&id) {
                        eprintln!("DragonGUI: dropping stale invalidate command for widget {id:?}");
                        ("stale_widget".to_string(), false)
                    } else {
                        gpu.rebuild_for_dirty(dirty);
                        ("applied".to_string(), true)
                    }
                };
                self.record_runtime_command(
                    "Invalidate",
                    Some(id),
                    None,
                    redraw.then_some(dirty),
                    &outcome,
                    redraw,
                )
            }
            Command::SetProp { id, prop, value } => {
                let detail = Some(format!("prop={prop}"));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "SetProp",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.apply_set_prop(&id, &prop, value) {
                        Some(dirty) => {
                            gpu.rebuild_for_dirty(dirty);
                            (Some(dirty), "applied".to_string(), true)
                        }
                        None => {
                            if !gpu.has_widget(&id) {
                                eprintln!(
                                    "DragonGUI: dropping stale SetProp command for widget {id:?}"
                                );
                                (None, "stale_widget".to_string(), false)
                            } else {
                                (None, "unsupported_or_noop".to_string(), false)
                            }
                        }
                    }
                };
                self.record_runtime_command("SetProp", Some(id), detail, dirty, &outcome, redraw)
            }
            Command::SetStyle { id, patch_json } => {
                let detail = Some(format!("patch_bytes={}", patch_json.len()));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "SetStyle",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.apply_set_style_patch(&id, &patch_json) {
                        Ok(Some(dirty)) => {
                            gpu.rebuild_for_dirty(dirty);
                            (Some(dirty), "applied".to_string(), true)
                        }
                        Ok(None) => {
                            if !gpu.has_widget(&id) {
                                eprintln!(
                                    "DragonGUI: dropping stale SetStyle command for widget {id:?}"
                                );
                                (None, "stale_widget".to_string(), false)
                            } else {
                                (None, "noop".to_string(), false)
                            }
                        }
                        Err(err) => {
                            eprintln!("DragonGUI: failed to apply style patch: {err}");
                            (None, format!("error: {err}"), false)
                        }
                    }
                };
                self.record_runtime_command("SetStyle", Some(id), detail, dirty, &outcome, redraw)
            }
            Command::ReplaceChildren { id, children_json } => {
                let detail = Some(format!("children_bytes={}", children_json.len()));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "ReplaceChildren",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.apply_replace_children(&id, &children_json) {
                        Ok(true) => {
                            gpu.apply_layout();
                            (Some(Dirty::Full), "applied".to_string(), true)
                        }
                        Ok(false) => {
                            eprintln!(
                                "DragonGUI: dropping stale ReplaceChildren command for widget {id:?}"
                            );
                            (None, "stale_widget".to_string(), false)
                        }
                        Err(err) => {
                            eprintln!("DragonGUI: failed to replace children: {err}");
                            (None, format!("error: {err}"), false)
                        }
                    }
                };
                self.record_runtime_command(
                    "ReplaceChildren",
                    Some(id),
                    detail,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::ReplaceNode { id, node_json } => {
                let detail = Some(format!("node_bytes={}", node_json.len()));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "ReplaceNode",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.apply_replace_node(&id, &node_json) {
                        Ok(true) => {
                            gpu.apply_layout();
                            (Some(Dirty::Full), "applied".to_string(), true)
                        }
                        Ok(false) => {
                            eprintln!(
                                "DragonGUI: dropping stale ReplaceNode command for widget {id:?}"
                            );
                            (None, "stale_widget".to_string(), false)
                        }
                        Err(err) => {
                            eprintln!("DragonGUI: failed to replace node: {err}");
                            (None, format!("error: {err}"), false)
                        }
                    }
                };
                self.record_runtime_command(
                    "ReplaceNode",
                    Some(id),
                    detail,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::SetScatterPointsPacked {
                id,
                xyz,
                telemetry,
                colormap,
            } => {
                let detail = Some(format!("payload_bytes={}, colormap={colormap}", xyz.len()));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "SetScatterPointsPacked",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.set_scatter_points_packed(&id, xyz, telemetry, colormap) {
                        Ok(true) => (Some(Dirty::GpuData), "applied".to_string(), true),
                        Ok(false) => {
                            eprintln!(
                                "DragonGUI: dropping stale scatter point update for widget {id:?}"
                            );
                            (None, "stale_widget".to_string(), false)
                        }
                        Err(err) => {
                            eprintln!("DragonGUI: failed to apply scatter point update: {err}");
                            (None, format!("error: {err}"), false)
                        }
                    }
                };
                self.record_runtime_command(
                    "SetScatterPointsPacked",
                    Some(id),
                    detail,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::SetTableData { id, table_json } => {
                let detail = Some(format!("table_bytes={}", table_json.len()));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "SetTableData",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.apply_set_table_data(&id, &table_json) {
                        Ok(true) => {
                            gpu.rebuild_for_dirty(Dirty::Text);
                            (Some(Dirty::Text), "applied".to_string(), true)
                        }
                        Ok(false) => {
                            eprintln!("DragonGUI: dropping stale table update for widget {id:?}");
                            (None, "stale_widget".to_string(), false)
                        }
                        Err(err) => {
                            eprintln!("DragonGUI: failed to apply table update: {err}");
                            (None, format!("error: {err}"), false)
                        }
                    }
                };
                self.record_runtime_command(
                    "SetTableData",
                    Some(id),
                    detail,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::SetTableDataColumns {
                id,
                table_json,
                columns,
            } => {
                let detail = Some(format!(
                    "table_bytes={}, buffer_columns={}",
                    table_json.len(),
                    columns.len()
                ));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "SetTableDataColumns",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.apply_set_table_data_columns(&id, &table_json, columns) {
                        Ok(true) => {
                            gpu.rebuild_for_dirty(Dirty::Text);
                            (Some(Dirty::Text), "applied".to_string(), true)
                        }
                        Ok(false) => {
                            eprintln!(
                                "DragonGUI: dropping stale table column update for widget {id:?}"
                            );
                            (None, "stale_widget".to_string(), false)
                        }
                        Err(err) => {
                            eprintln!("DragonGUI: failed to apply table column update: {err}");
                            (None, format!("error: {err}"), false)
                        }
                    }
                };
                self.record_runtime_command(
                    "SetTableDataColumns",
                    Some(id),
                    detail,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::SetBufferResource {
                id,
                kind,
                bytes,
                owner_id,
            } => {
                let detail = Some(format!(
                    "kind={kind}, bytes={}, owner={}",
                    bytes.len(),
                    owner_id.as_deref().unwrap_or("<app>")
                ));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "SetBufferResource",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    gpu.apply_set_buffer_resource(&id, &kind, bytes, owner_id);
                    (Some(Dirty::GpuData), "applied".to_string(), true)
                };
                self.record_runtime_command(
                    "SetBufferResource",
                    Some(id),
                    detail,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::ReleaseResource { id } => {
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "ReleaseResource",
                            Some(id),
                            None,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    if gpu.apply_release_resource(&id) {
                        gpu.apply_layout();
                        (Some(Dirty::Full), "released".to_string(), true)
                    } else {
                        eprintln!("DragonGUI: dropping stale resource release for id {id:?}");
                        (None, "stale_resource".to_string(), false)
                    }
                };
                self.record_runtime_command(
                    "ReleaseResource",
                    Some(id),
                    None,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::DebugSnapshot { request_id } => {
                self.record_runtime_command(
                    "DebugSnapshot",
                    None,
                    Some(format!("request_id={request_id}")),
                    None,
                    "completed",
                    false,
                );
                if let Some(bridge) = &self.command_bridge {
                    bridge.complete_debug_snapshot(request_id, self.debug_snapshot_json());
                }
                false
            }
        }
    }

    fn emit_change(&self, id: &str, value: ChangeValue) {
        if let Some(cb) = self.change_cbs.get(id) {
            cb(value);
        } else if let Some(handle) = &self.python_runtime {
            Python::with_gil(|py| {
                let result = match value {
                    ChangeValue::Bool(v) => {
                        handle.call_method1(py, "_invoke_change_callback", (id, v))
                    }
                    ChangeValue::Float(v) => {
                        handle.call_method1(py, "_invoke_change_callback", (id, v))
                    }
                    ChangeValue::Text(v) => {
                        handle.call_method1(py, "_invoke_change_callback", (id, v))
                    }
                };
                if let Err(err) = result {
                    err.print(py);
                }
            });
        }
    }

    fn emit_click(&self, id: &str) {
        if let Some(cb) = self.click_cbs.get(id) {
            cb();
        } else if let Some(handle) = &self.python_runtime {
            Python::with_gil(|py| {
                if let Err(err) = handle.call_method1(py, "_invoke_click_callback", (id,)) {
                    err.print(py);
                }
            });
        }
    }

    fn current_slider_value(&self, id: &str) -> Option<f32> {
        self.gpu
            .as_ref()
            .and_then(|g| g.widget_state.as_ref())
            .and_then(|ws| ws.float_val.get(id).copied())
    }

    fn emit_slider_change(&mut self, id: &str, value: f32, force: bool) {
        let duplicate = match &self.last_slider_emit {
            Some(last) => {
                last.widget_id == id && (last.value - value).abs() <= SLIDER_CHANGE_EPSILON
            }
            None => false,
        };
        if duplicate {
            return;
        }

        let now = Instant::now();
        let can_emit = force
            || match &self.last_slider_emit {
                Some(last) if last.widget_id == id => {
                    now.duration_since(last.at) >= SLIDER_CALLBACK_INTERVAL
                }
                _ => true,
            };

        if can_emit {
            if matches!(
                self.pending_slider_emit.as_ref(),
                Some((pending_id, _)) if pending_id == id
            ) {
                self.pending_slider_emit = None;
            }
            self.last_slider_emit = Some(SliderChangeDispatch {
                widget_id: id.to_string(),
                value,
                at: now,
            });
            self.emit_change(id, ChangeValue::Float(value));
        } else {
            self.pending_slider_emit = Some((id.to_string(), value));
        }
    }

    fn flush_slider_change(&mut self, id: &str) {
        let pending = self.pending_slider_emit.take();
        let mut restore_pending = None;
        let value = match pending {
            Some((pending_id, pending_value)) if pending_id == id => Some(pending_value),
            Some(other) => {
                restore_pending = Some(other);
                self.current_slider_value(id)
            }
            None => self.current_slider_value(id),
        };
        self.pending_slider_emit = restore_pending;

        if let Some(value) = value {
            self.emit_slider_change(id, value, true);
        }
    }

    fn update_slider_drag(&mut self, mouse_x: f32, force_emit: bool) {
        let (id, new_val) = match &self.slider_drag {
            Some(drag) => (drag.widget_id.clone(), drag.compute_value(mouse_x)),
            None => return,
        };

        let mut changed_val = new_val;
        let mut changed = false;
        if let Some(gpu) = &mut self.gpu {
            if let Some(ws) = &mut gpu.widget_state {
                let old = ws.float_val.get(&id).copied();
                changed_val = ws.set_float(&id, new_val);
                changed = old
                    .map(|old| (old - changed_val).abs() > SLIDER_CHANGE_EPSILON)
                    .unwrap_or(true);
            }
            if changed {
                gpu.rebuild_primitives();
            }
        }

        if changed {
            self.emit_slider_change(&id, changed_val, force_emit);
            self.request_redraw();
        }
    }

    fn activate_widget(&mut self, id: &str, kind: WidgetKind) {
        let needs_text_rebuild = kind == WidgetKind::Dropdown;
        let mut needs_layout_rebuild = false;
        let mut navigation_change: Option<(String, String)> = None;
        match kind {
            WidgetKind::Button => {
                self.emit_click(id);
            }
            WidgetKind::Checkbox => {
                let new_val = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .map(|ws| ws.toggle_checkbox(id))
                    .unwrap_or(false);
                self.emit_change(id, ChangeValue::Bool(new_val));
            }
            WidgetKind::Dropdown => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.toggle_dropdown(id);
                    }
                }
            }
            WidgetKind::Tab => {
                navigation_change = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.activate_tab(id));
                needs_layout_rebuild = navigation_change.is_some();
            }
            WidgetKind::NavItem => {
                navigation_change = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.activate_nav_item(id));
                needs_layout_rebuild = navigation_change.is_some();
            }
            _ => {}
        }

        if let Some((owner, value)) = navigation_change {
            self.emit_change(&owner, ChangeValue::Text(value));
        }
        if let Some(gpu) = &mut self.gpu {
            if needs_layout_rebuild {
                gpu.apply_layout();
            } else if needs_text_rebuild {
                gpu.rebuild_visuals();
            } else {
                gpu.rebuild_primitives();
            }
        }
        self.request_redraw();
    }

    fn select_dropdown_option(&mut self, id: &str, idx: usize) {
        let selected = self
            .gpu
            .as_mut()
            .and_then(|g| g.widget_state.as_mut())
            .and_then(|ws| ws.select_dropdown_index(id, idx));
        if let Some(value) = selected {
            self.emit_change(id, ChangeValue::Text(value));
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.rebuild_visuals();
        }
        self.request_redraw();
    }

    fn commit_navigation_change(&mut self, change: Option<(String, String, String)>) -> bool {
        let Some((owner, value, _focus_id)) = change else {
            return false;
        };
        self.emit_change(&owner, ChangeValue::Text(value));
        if let Some(gpu) = &mut self.gpu {
            gpu.apply_layout();
        }
        self.request_redraw();
        true
    }

    fn select_table_cell(&mut self, id: &str, row: usize, col: usize) {
        if let Some(gpu) = &mut self.gpu {
            if let Some(ws) = &mut gpu.widget_state {
                ws.select_table_cell(id, row, col);
            }
            gpu.rebuild_visuals();
        }
        self.request_redraw();
    }

    fn toggle_table_sort(&mut self, id: &str, col: usize) {
        if let Some(gpu) = &mut self.gpu {
            if let Some(ws) = &mut gpu.widget_state {
                ws.toggle_table_sort(id, col);
            }
            gpu.rebuild_visuals();
        }
        self.request_redraw();
    }

    fn scroll_table(&mut self, id: &str, row_delta: isize, col_delta: isize) {
        if let Some(gpu) = &mut self.gpu {
            let changed = gpu
                .widget_state
                .as_mut()
                .map(|ws| ws.scroll_table(id, row_delta, col_delta))
                .unwrap_or(false);
            if changed {
                gpu.rebuild_visuals();
                self.request_redraw();
            }
        }
    }

    fn set_focus(&mut self, id: Option<String>) {
        if let Some(gpu) = &mut self.gpu {
            gpu.focus_widget(id);
        }
        let text_rect = self
            .gpu
            .as_ref()
            .and_then(|gpu| gpu.focused_text_input_rect());
        if let Some(window) = &self.window {
            window.set_ime_allowed(text_rect.is_some());
            if let Some(rect) = text_rect {
                window.set_ime_cursor_area(
                    PhysicalPosition::new(rect.x as i32, rect.y as i32),
                    PhysicalSize::new(rect.w.max(1.0) as u32, rect.h.max(1.0) as u32),
                );
            }
        }
        self.request_redraw();
    }

    fn handle_keyboard_input(&mut self, event: winit::event::KeyEvent) {
        if matches!(&event.logical_key, Key::Named(NamedKey::Tab)) {
            if let Some(gpu) = &mut self.gpu {
                if let (Some(ws), Some(layout)) =
                    (gpu.widget_state.as_mut(), gpu.current_layout.as_ref())
                {
                    ws.focus_next_visible(layout, self.modifiers.shift_key());
                }
                gpu.rebuild_visuals();
            }
            self.request_redraw();
            return;
        }

        let focused = self.gpu.as_ref().and_then(|g| g.focused_kind());
        if let Some((id, kind)) = focused {
            match kind {
                WidgetKind::TextInput => {
                    if self.handle_text_input_key(&id, &event) {
                        return;
                    }
                }
                WidgetKind::Slider => {
                    let dir = match &event.logical_key {
                        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowDown) => {
                            Some(-1.0)
                        }
                        Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowUp) => {
                            Some(1.0)
                        }
                        _ => None,
                    };
                    if let Some(dir) = dir {
                        let changed = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| ws.adjust_float(&id, dir));
                        if let Some(value) = changed {
                            self.emit_change(&id, ChangeValue::Float(value));
                        }
                        if let Some(gpu) = &mut self.gpu {
                            gpu.rebuild_primitives();
                        }
                        self.request_redraw();
                        return;
                    }
                }
                WidgetKind::Dropdown => match &event.logical_key {
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        self.activate_widget(&id, WidgetKind::Dropdown);
                        return;
                    }
                    Key::Named(NamedKey::Escape) => {
                        if let Some(gpu) = &mut self.gpu {
                            if let Some(ws) = &mut gpu.widget_state {
                                ws.set_dropdown_open(None);
                            }
                            gpu.rebuild_visuals();
                        }
                        self.request_redraw();
                        return;
                    }
                    Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown) => {
                        let direction =
                            if matches!(&event.logical_key, Key::Named(NamedKey::ArrowUp)) {
                                -1
                            } else {
                                1
                            };
                        let selected = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| {
                                ws.set_dropdown_open(Some(id.clone()));
                                ws.move_dropdown_index(&id, direction)
                            });
                        if let Some(value) = selected {
                            self.emit_change(&id, ChangeValue::Text(value));
                        }
                        if let Some(gpu) = &mut self.gpu {
                            gpu.rebuild_visuals();
                        }
                        self.request_redraw();
                        return;
                    }
                    _ => {}
                },
                WidgetKind::Button | WidgetKind::Checkbox => {
                    if matches!(
                        &event.logical_key,
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space)
                    ) {
                        self.activate_widget(&id, kind);
                        return;
                    }
                }
                WidgetKind::Tab => match &event.logical_key {
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        self.activate_widget(&id, WidgetKind::Tab);
                        return;
                    }
                    Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight) => {
                        let direction =
                            if matches!(&event.logical_key, Key::Named(NamedKey::ArrowLeft)) {
                                -1
                            } else {
                                1
                            };
                        let change = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| ws.move_tab(&id, direction));
                        if self.commit_navigation_change(change) {
                            return;
                        }
                    }
                    Key::Named(NamedKey::Home) | Key::Named(NamedKey::End) => {
                        let end = matches!(&event.logical_key, Key::Named(NamedKey::End));
                        let change = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| ws.move_tab_edge(&id, end));
                        if self.commit_navigation_change(change) {
                            return;
                        }
                    }
                    _ => {}
                },
                WidgetKind::NavItem => match &event.logical_key {
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                        self.activate_widget(&id, WidgetKind::NavItem);
                        return;
                    }
                    Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown) => {
                        let direction =
                            if matches!(&event.logical_key, Key::Named(NamedKey::ArrowUp)) {
                                -1
                            } else {
                                1
                            };
                        let change = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| ws.move_nav_item(&id, direction));
                        if self.commit_navigation_change(change) {
                            return;
                        }
                    }
                    Key::Named(NamedKey::Home) | Key::Named(NamedKey::End) => {
                        let end = matches!(&event.logical_key, Key::Named(NamedKey::End));
                        let change = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| ws.move_nav_item_edge(&id, end));
                        if self.commit_navigation_change(change) {
                            return;
                        }
                    }
                    _ => {}
                },
                WidgetKind::DataFrameTable => {
                    let movement = match &event.logical_key {
                        Key::Named(NamedKey::ArrowUp) => Some((-1, 0)),
                        Key::Named(NamedKey::ArrowDown) => Some((1, 0)),
                        Key::Named(NamedKey::ArrowLeft) => Some((0, -1)),
                        Key::Named(NamedKey::ArrowRight) => Some((0, 1)),
                        Key::Named(NamedKey::PageUp) => Some((-10, 0)),
                        Key::Named(NamedKey::PageDown) => Some((10, 0)),
                        _ => None,
                    };
                    if let Some((row_delta, col_delta)) = movement {
                        let visible_counts = self
                            .gpu
                            .as_ref()
                            .and_then(|g| g.table_visible_counts(&id))
                            .unwrap_or((1, 1));
                        if let Some(gpu) = &mut self.gpu {
                            if let Some(ws) = &mut gpu.widget_state {
                                ws.move_table_selection(
                                    &id,
                                    row_delta,
                                    col_delta,
                                    visible_counts.0,
                                    visible_counts.1,
                                );
                            }
                            gpu.rebuild_visuals();
                        }
                        self.request_redraw();
                        return;
                    }
                }
                _ => {}
            }
        }

        let reset = matches!(
            event.physical_key,
            PhysicalKey::Code(KeyCode::KeyR) | PhysicalKey::Code(KeyCode::Home)
        );
        if reset {
            if let Some(gpu) = &mut self.gpu {
                if let Some(s) = &mut gpu.scatter {
                    s.reset_camera(&gpu.queue);
                    self.request_redraw();
                }
            }
        }
    }

    fn handle_text_input_key(&mut self, id: &str, event: &winit::event::KeyEvent) -> bool {
        let mut changed: Option<String> = None;
        let mut handled = true;
        match &event.logical_key {
            Key::Named(NamedKey::Backspace) => {
                changed = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.backspace_text(id));
            }
            Key::Named(NamedKey::Delete) => {
                changed = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.delete_text(id));
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.move_text_cursor(id, -1);
                    }
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.move_text_cursor(id, 1);
                    }
                }
            }
            Key::Named(NamedKey::Home) => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.move_text_cursor_home_end(id, false);
                    }
                }
            }
            Key::Named(NamedKey::End) => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.move_text_cursor_home_end(id, true);
                    }
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.set_focus(None);
                return true;
            }
            _ => {
                handled = false;
            }
        }

        if !handled
            && !self.modifiers.control_key()
            && !self.modifiers.alt_key()
            && !self.modifiers.super_key()
        {
            if let Some(text) = event.text.as_deref().filter(|text| is_insert_text(text)) {
                changed = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.insert_text(id, text));
                handled = true;
            }
        }

        if let Some(value) = changed {
            self.emit_change(id, ChangeValue::Text(value));
        }
        if handled {
            if let Some(gpu) = &mut self.gpu {
                gpu.rebuild_visuals();
            }
            self.request_redraw();
        }
        handled
    }
}

fn is_insert_text(text: &str) -> bool {
    !text.is_empty()
        && !text
            .chars()
            .any(|ch| ch == '\r' || ch == '\n' || ch == '\t' || ch.is_control())
}

impl ApplicationHandler<RuntimeEvent> for DragonApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut spec = match self.spec.take() {
            Some(s) => s,
            None => return,
        };

        // Move callbacks out of spec before passing spec to WgpuState::new.
        self.click_cbs = std::mem::take(&mut spec.click_callbacks);
        self.change_cbs = std::mem::take(&mut spec.change_callbacks);

        let attrs = Window::default_attributes()
            .with_title(&spec.title)
            .with_inner_size(LogicalSize::new(spec.width, spec.height));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(DragonError::GpuInit(format!("window creation: {e}")));
                event_loop.exit();
                return;
            }
        };

        match pollster::block_on(WgpuState::new(Arc::clone(&window), spec)) {
            Ok((gpu, upload_ms)) => {
                self.upload_ms = upload_ms;
                self.gpu = Some(gpu);
                self.window = Some(window);
                self.drain_runtime_commands();
                self.request_redraw();
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Wake => self.drain_runtime_commands(),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
                self.request_redraw();
            }

            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer: _,
            } => {
                let new_size = self
                    .window
                    .as_ref()
                    .map(|w| w.inner_size())
                    .unwrap_or_default();
                if let Some(gpu) = &mut self.gpu {
                    gpu.set_scale_factor(scale_factor, new_size);
                }
                self.request_redraw();
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;

                match button {
                    MouseButton::Left => {
                        if !pressed {
                            // ── release ───────────────────────────────────────
                            self.orbit_active = false;
                            let released_slider =
                                self.slider_drag.as_ref().map(|drag| drag.widget_id.clone());
                            if let Some(id) = released_slider {
                                self.flush_slider_change(&id);
                            }
                            self.slider_drag = None;

                            if let Some(pid) = self.pressed_id.take() {
                                // Clear pressed visual state.
                                if let Some(gpu) = &mut self.gpu {
                                    if let Some(ws) = &mut gpu.widget_state {
                                        ws.pressed = None;
                                    }
                                }
                                // Fire callback only if released over the same widget.
                                let pos = self.last_mouse_pos.unwrap_or([0.0, 0.0]);
                                let over = self
                                    .gpu
                                    .as_ref()
                                    .and_then(|g| g.hit_test_ui(pos))
                                    .map(|(id, _)| id);
                                if over.as_deref() == Some(&pid) {
                                    if let Some(kind) =
                                        self.gpu.as_ref().and_then(|g| g.widget_kind(&pid))
                                    {
                                        self.activate_widget(&pid, kind);
                                    }
                                }
                                // Rebuild to clear pressed / update checkbox.
                                if let Some(gpu) = &mut self.gpu {
                                    gpu.rebuild_primitives();
                                }
                                self.request_redraw();
                            }
                        } else {
                            // ── press ─────────────────────────────────────────
                            let pos = self.last_mouse_pos.unwrap_or([0.0, 0.0]);
                            if let Some((id, idx)) =
                                self.gpu.as_ref().and_then(|g| g.dropdown_option_at(pos))
                            {
                                self.set_focus(Some(id.clone()));
                                self.select_dropdown_option(&id, idx);
                                return;
                            }

                            if let Some((id, hit)) =
                                self.gpu.as_ref().and_then(|g| g.table_hit(pos))
                            {
                                self.set_focus(Some(id.clone()));
                                match hit {
                                    TableHit::Header(col) => self.toggle_table_sort(&id, col),
                                    TableHit::Cell { row, col } => {
                                        self.select_table_cell(&id, row, col)
                                    }
                                }
                                return;
                            }

                            if let Some((id, kind)) =
                                self.gpu.as_ref().and_then(|g| g.hit_test_ui(pos))
                            {
                                self.set_focus(Some(id.clone()));
                                self.pressed_id = Some(id.clone());
                                if let Some(gpu) = &mut self.gpu {
                                    if let Some(ws) = &mut gpu.widget_state {
                                        ws.pressed = Some(id.clone());
                                    }
                                    if kind == WidgetKind::Slider {
                                        self.slider_drag = gpu.create_slider_drag(&id);
                                    }
                                    gpu.rebuild_primitives();
                                }
                                if kind == WidgetKind::Slider {
                                    self.update_slider_drag(pos[0], true);
                                }
                                self.request_redraw();
                                return;
                            }

                            let over_scatter = self
                                .gpu
                                .as_ref()
                                .map(|g| g.scatter_contains(pos))
                                .unwrap_or(false);

                            if over_scatter {
                                self.set_focus(None);
                                self.orbit_active = true;
                            } else {
                                self.set_focus(None);
                            }
                        }
                    }

                    MouseButton::Middle | MouseButton::Right => {
                        if !pressed {
                            self.pan_active = false;
                        } else {
                            let pos = self.last_mouse_pos.unwrap_or([0.0, 0.0]);
                            self.pan_active = self
                                .gpu
                                .as_ref()
                                .map(|g| g.scatter_contains(pos))
                                .unwrap_or(false);
                        }
                    }

                    _ => {}
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = [position.x as f32, position.y as f32];

                // Slider drag takes priority.
                if self.slider_drag.is_some() {
                    self.update_slider_drag(new_pos[0], false);
                } else if let Some(old) = self.last_mouse_pos {
                    let delta = glam::Vec2::new(new_pos[0] - old[0], new_pos[1] - old[1]);
                    if let Some(gpu) = &mut self.gpu {
                        if let Some(s) = &mut gpu.scatter {
                            if self.orbit_active {
                                s.camera.orbit(delta);
                                s.update_camera(&gpu.queue);
                                self.request_redraw();
                            } else if self.pan_active {
                                s.camera.pan(delta);
                                s.update_camera(&gpu.queue);
                                self.request_redraw();
                            }
                        }
                    }
                }

                // Update hover state when no button is held.
                if self.slider_drag.is_none() && !self.orbit_active && !self.pan_active {
                    let new_hover = self
                        .gpu
                        .as_ref()
                        .and_then(|g| g.hit_test_ui(new_pos))
                        .map(|(id, _)| id);
                    let old_hover = self
                        .gpu
                        .as_ref()
                        .and_then(|g| g.widget_state.as_ref())
                        .and_then(|ws| ws.hovered.clone());
                    if new_hover != old_hover {
                        if let Some(gpu) = &mut self.gpu {
                            if let Some(ws) = &mut gpu.widget_state {
                                ws.hovered = new_hover;
                            }
                            gpu.rebuild_primitives();
                        }
                        self.request_redraw();
                    }
                }

                self.last_mouse_pos = Some(new_pos);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (scroll_x, scroll_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 * 0.01, pos.y as f32 * 0.01),
                };
                if let Some(pos) = self.last_mouse_pos {
                    if let Some(id) = self.gpu.as_ref().and_then(|gpu| gpu.table_at(pos)) {
                        let row_delta = (-(scroll_y * 3.0)).round() as isize;
                        let col_delta = if scroll_x.abs() >= 0.5 {
                            (-scroll_x).round() as isize
                        } else if self.modifiers.shift_key() {
                            (-scroll_y).round() as isize
                        } else {
                            0
                        };
                        self.scroll_table(&id, row_delta, col_delta);
                        return;
                    }
                }
                let over_scatter = self
                    .last_mouse_pos
                    .and_then(|pos| self.gpu.as_ref().map(|gpu| gpu.scatter_contains(pos)))
                    .unwrap_or(false);
                if !over_scatter {
                    return;
                }
                if let Some(gpu) = &mut self.gpu {
                    if let Some(s) = &mut gpu.scatter {
                        s.camera.zoom(scroll_y);
                        s.update_camera(&gpu.queue);
                        self.request_redraw();
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.handle_keyboard_input(event);
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::Ime(Ime::Commit(text)) => {
                let focused = self.gpu.as_ref().and_then(|g| g.focused_kind());
                if let Some((id, WidgetKind::TextInput)) = focused {
                    if is_insert_text(&text) {
                        let changed = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| ws.insert_text(&id, &text));
                        if let Some(value) = changed {
                            self.emit_change(&id, ChangeValue::Text(value));
                        }
                        if let Some(gpu) = &mut self.gpu {
                            gpu.rebuild_visuals();
                        }
                        self.request_redraw();
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &mut self.gpu {
                    let t0 = Instant::now();
                    if let Err(e) = gpu.render() {
                        self.error = Some(e);
                        event_loop.exit();
                        return;
                    }
                    self.frame_ms_total += t0.elapsed().as_secs_f64() * 1000.0;
                    self.frames_rendered += 1;
                }
                if let Some(limit) = self.smoke_frames {
                    if self.frames_rendered >= limit {
                        event_loop.exit();
                    } else {
                        self.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run_event_loop(spec: AppSpec) -> Result<RunResult, DragonError> {
    let smoke_frames = std::env::var("DRAGONGUI_SMOKE_FRAMES")
        .ok()
        .and_then(|v| v.parse::<u32>().ok());

    let event_loop = EventLoop::<RuntimeEvent>::with_user_event()
        .build()
        .map_err(|e| DragonError::GpuInit(format!("event loop: {e}")))?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = DragonApp::new(spec, smoke_frames);
    if let Some(bridge) = &app.command_bridge {
        bridge.install_proxy(event_loop.create_proxy());
    }
    let event_loop_result = event_loop
        .run_app(&mut app)
        .map_err(|e| DragonError::GpuInit(format!("event loop error: {e}")));

    if let Some(bridge) = &app.command_bridge {
        bridge.close();
    }

    event_loop_result?;

    if let Some(e) = app.take_error() {
        return Err(e);
    }

    Ok(app.run_result())
}
