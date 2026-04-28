use std::collections::{HashMap, HashSet, VecDeque};
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
use crate::css_style::{
    apply_stylesheets_to_tree, matched_part_rule_labels_for_tree, matched_rule_labels_for_tree,
    StylesheetOrigin, StylesheetStore,
};
use crate::document::{self, NodeProps, ScatterSpec, WidgetKind, WidgetNode};
use crate::error::DragonError;
use crate::events::{
    has_active_modal, hit_test, hit_test_hover, modal_blocks_point, ChangeValue, SliderDrag,
    WidgetState,
};
use crate::image_widget::ImageRenderer;
use crate::layout::{compute_layout, is_scroll_container_node, scroll_container_max_y};
use crate::overlays::menu_popup_width;
use crate::primitives::PrimitivesRenderer;
use crate::resources::ResourceRegistry;
use crate::scatter::{self, PointInstance, ScatterWidget};
use crate::style::{
    collapsible_header_height_for_style, number_stepper_width, number_stepper_width_for_style,
    BackgroundPaint, BoxShadow, ColorRef, DisplayStyle, FlexDirectionStyle, FontFamily, FontStyle,
    FontVariantNumeric, GridLineStyle, GridPlacementStyle, GridTrackSize, LayoutLength,
    LayoutStyle, LineHeight, NodeStyle, OverflowStyle, PartLayoutStyle, PartStyle, PositionStyle,
    TextAlign, TextOverflow, TextSpacing, TextStyle, TextTransform, TransitionStyle,
    TransitionTimingFunction, VisualStyle, WidgetStyle,
};
use crate::table::{self, TableHit};
use crate::text::TextRendererDg;
use crate::theme::Theme;
use crate::toast::{ToastLevel, ToastOverlay, ToastPosition};

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
    /// Parsed startup stylesheets. Cascade/render integration is added in later CSS milestones.
    pub stylesheets: StylesheetStore,
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
    if node.kind == WidgetKind::Scatter3D && layout.visible_rect(&node.id).is_some() {
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

fn find_widget<'a>(node: &'a WidgetNode, id: &str) -> Option<&'a WidgetNode> {
    if node.id == id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_widget(child, id) {
            return Some(found);
        }
    }
    None
}

fn scroll_container_at_pos(
    node: &WidgetNode,
    layout: &crate::layout::LayoutResult,
    state: &WidgetState,
    pos: [f32; 2],
) -> Option<String> {
    if node.kind == WidgetKind::Tooltip {
        return None;
    }
    for child in node.children.iter().rev() {
        if let Some(id) = scroll_container_at_pos(child, layout, state, pos) {
            return Some(id);
        }
    }
    if !is_scroll_container_node(node) || state.is_disabled(&node.id) {
        return None;
    }
    let rect = layout.visible_rect(&node.id)?;
    if pos[0] < rect.x || pos[0] >= rect.x + rect.w || pos[1] < rect.y || pos[1] >= rect.y + rect.h
    {
        return None;
    }
    let max_scroll = layout
        .scroll_max_y
        .get(&node.id)
        .copied()
        .unwrap_or_else(|| scroll_container_max_y(node, layout));
    (max_scroll > 0.0).then(|| node.id.clone())
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

fn close_active_modal(node: &mut WidgetNode) -> Option<String> {
    for child in node.children.iter_mut().rev() {
        if let Some(id) = close_active_modal(child) {
            return Some(id);
        }
    }
    if node.kind == WidgetKind::Modal && node.props.open.unwrap_or(false) {
        node.props.open = Some(false);
        return Some(node.id.clone());
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
        WidgetKind::Collapsible => "collapsible",
        WidgetKind::Modal => "modal",
        WidgetKind::Badge => "badge",
        WidgetKind::Tag => "tag",
        WidgetKind::Button => "button",
        WidgetKind::Checkbox => "checkbox",
        WidgetKind::Dropdown => "dropdown",
        WidgetKind::Label => "label",
        WidgetKind::Slider => "slider",
        WidgetKind::NumberInput => "number_input",
        WidgetKind::ProgressBar => "progress_bar",
        WidgetKind::TextInput => "text_input",
        WidgetKind::TextArea => "text_area",
        WidgetKind::Separator => "separator",
        WidgetKind::Spacer => "spacer",
        WidgetKind::StatusBar => "status_bar",
        WidgetKind::MenuBar => "menu_bar",
        WidgetKind::Menu => "menu",
        WidgetKind::MenuItem => "menu_item",
        WidgetKind::ContextMenu => "context_menu",
        WidgetKind::Tooltip => "tooltip",
        WidgetKind::Toast => "toast",
        WidgetKind::Tabs => "tabs",
        WidgetKind::Tab => "tab",
        WidgetKind::Pages => "pages",
        WidgetKind::Page => "page",
        WidgetKind::Sidebar => "sidebar",
        WidgetKind::NavItem => "nav_item",
        WidgetKind::Scatter3D => "scatter_3d",
        WidgetKind::DataFrameTable => "dataframe_table",
        WidgetKind::Image => "image",
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

fn color_ref_json(color: &ColorRef) -> Value {
    match color {
        ColorRef::Rgba(color) => json!({ "rgba": color_json(*color) }),
        ColorRef::Token(token) => json!({ "token": token }),
    }
}

fn display_style_name(display: DisplayStyle) -> &'static str {
    match display {
        DisplayStyle::Flex => "flex",
        DisplayStyle::Grid => "grid",
        DisplayStyle::Block => "block",
        DisplayStyle::None => "none",
    }
}

fn flex_direction_name(direction: FlexDirectionStyle) -> &'static str {
    match direction {
        FlexDirectionStyle::Row => "row",
        FlexDirectionStyle::Column => "column",
        FlexDirectionStyle::RowReverse => "row_reverse",
        FlexDirectionStyle::ColumnReverse => "column_reverse",
    }
}

fn overflow_style_name(value: OverflowStyle) -> &'static str {
    match value {
        OverflowStyle::Visible => "visible",
        OverflowStyle::Hidden => "hidden",
        OverflowStyle::Scroll => "scroll",
        OverflowStyle::Auto => "auto",
    }
}

fn position_style_name(value: PositionStyle) -> &'static str {
    match value {
        PositionStyle::Static => "static",
        PositionStyle::Relative => "relative",
        PositionStyle::Absolute => "absolute",
    }
}

fn text_align_name(align: TextAlign) -> &'static str {
    match align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

fn text_transform_name(value: TextTransform) -> &'static str {
    match value {
        TextTransform::None => "none",
        TextTransform::Uppercase => "uppercase",
        TextTransform::Lowercase => "lowercase",
        TextTransform::Capitalize => "capitalize",
    }
}

fn font_style_name(value: FontStyle) -> &'static str {
    match value {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
    }
}

fn font_variant_numeric_name(value: FontVariantNumeric) -> &'static str {
    match value {
        FontVariantNumeric::Normal => "normal",
        FontVariantNumeric::TabularNums => "tabular_nums",
    }
}

fn text_overflow_name(value: TextOverflow) -> &'static str {
    match value {
        TextOverflow::Clip => "clip",
        TextOverflow::Ellipsis => "ellipsis",
    }
}

fn text_spacing_json(value: TextSpacing) -> Value {
    match value {
        TextSpacing::LogicalPx(value) => json!({ "px": value }),
        TextSpacing::Em(value) => json!({ "em": value }),
    }
}

fn line_height_json(value: LineHeight) -> Value {
    match value {
        LineHeight::Multiplier(value) => json!({ "multiplier": value }),
        LineHeight::LogicalPx(value) => json!({ "px": value }),
    }
}

fn font_family_json(font_family: &FontFamily) -> Value {
    match font_family {
        FontFamily::Serif => json!("serif"),
        FontFamily::SansSerif => json!("sans_serif"),
        FontFamily::Monospace => json!("monospace"),
        FontFamily::Cursive => json!("cursive"),
        FontFamily::Fantasy => json!("fantasy"),
        FontFamily::Name(name) => json!(name),
    }
}

fn insert_number(map: &mut Map<String, Value>, key: &str, value: Option<f32>) {
    if let Some(value) = value {
        map.insert(key.to_string(), json!(value));
    }
}

fn insert_layout_length(
    map: &mut Map<String, Value>,
    key: &str,
    value: Option<LayoutLength>,
    legacy_px: Option<f32>,
) {
    match value {
        Some(LayoutLength::LogicalPx(value)) => {
            map.insert(key.to_string(), json!(value));
        }
        Some(LayoutLength::Percent(value)) => {
            map.insert(key.to_string(), json!({ "percent": value }));
        }
        Some(LayoutLength::Calc(value)) => {
            map.insert(
                key.to_string(),
                json!({ "calc": { "percent": value.percent, "px": value.px } }),
            );
        }
        Some(LayoutLength::Auto) => {
            map.insert(key.to_string(), json!("auto"));
        }
        None => insert_number(map, key, legacy_px),
    }
}

fn grid_track_json(value: GridTrackSize) -> Value {
    match value {
        GridTrackSize::LogicalPx(value) => json!(value),
        GridTrackSize::Percent(value) => json!({ "percent": value }),
        GridTrackSize::Fraction(value) => json!({ "fr": value }),
        GridTrackSize::Auto => json!("auto"),
    }
}

fn grid_line_json(value: GridLineStyle) -> Value {
    match value {
        GridLineStyle::Auto => json!("auto"),
        GridLineStyle::Line(value) => json!(value),
        GridLineStyle::Span(value) => json!({ "span": value }),
    }
}

fn grid_placement_json(value: GridPlacementStyle) -> Value {
    json!({
        "start": grid_line_json(value.start),
        "end": grid_line_json(value.end),
    })
}

fn insert_color_ref(map: &mut Map<String, Value>, key: &str, value: &Option<ColorRef>) {
    if let Some(value) = value {
        map.insert(key.to_string(), color_ref_json(value));
    }
}

fn layout_style_snapshot(style: &LayoutStyle) -> Value {
    let mut map = Map::new();
    if let Some(value) = style.display {
        map.insert("display".to_string(), json!(display_style_name(value)));
    }
    if let Some(value) = style.flex_direction {
        map.insert(
            "flex_direction".to_string(),
            json!(flex_direction_name(value)),
        );
    }
    insert_layout_length(&mut map, "width", style.width_value, style.width);
    insert_layout_length(&mut map, "height", style.height_value, style.height);
    insert_layout_length(
        &mut map,
        "min_width",
        style.min_width_value,
        style.min_width,
    );
    insert_layout_length(
        &mut map,
        "min_height",
        style.min_height_value,
        style.min_height,
    );
    insert_layout_length(
        &mut map,
        "max_width",
        style.max_width_value,
        style.max_width,
    );
    insert_layout_length(
        &mut map,
        "max_height",
        style.max_height_value,
        style.max_height,
    );
    insert_number(&mut map, "padding", style.padding);
    insert_number(&mut map, "padding_left", style.padding_left);
    insert_number(&mut map, "padding_right", style.padding_right);
    insert_number(&mut map, "padding_top", style.padding_top);
    insert_number(&mut map, "padding_bottom", style.padding_bottom);
    insert_number(&mut map, "margin", style.margin);
    insert_number(&mut map, "gap", style.gap);
    insert_number(&mut map, "row_gap", style.row_gap);
    insert_number(&mut map, "column_gap", style.column_gap);
    if let Some(value) = style.overflow {
        map.insert("overflow".to_string(), json!(overflow_style_name(value)));
    }
    if let Some(value) = style.overflow_x {
        map.insert("overflow_x".to_string(), json!(overflow_style_name(value)));
    }
    if let Some(value) = style.overflow_y {
        map.insert("overflow_y".to_string(), json!(overflow_style_name(value)));
    }
    if let Some(value) = style.position {
        map.insert("position".to_string(), json!(position_style_name(value)));
    }
    insert_number(&mut map, "top", style.top);
    insert_number(&mut map, "right", style.right);
    insert_number(&mut map, "bottom", style.bottom);
    insert_number(&mut map, "left", style.left);
    if let Some(value) = style.z_index {
        map.insert("z_index".to_string(), json!(value));
    }
    insert_number(&mut map, "flex_grow", style.flex_grow);
    insert_number(&mut map, "flex_shrink", style.flex_shrink);
    if let Some(value) = &style.grid_template_columns {
        map.insert(
            "grid_template_columns".to_string(),
            Value::Array(value.iter().copied().map(grid_track_json).collect()),
        );
    }
    if let Some(value) = &style.grid_template_rows {
        map.insert(
            "grid_template_rows".to_string(),
            Value::Array(value.iter().copied().map(grid_track_json).collect()),
        );
    }
    if let Some(value) = style.grid_column {
        map.insert("grid_column".to_string(), grid_placement_json(value));
    }
    if let Some(value) = style.grid_row {
        map.insert("grid_row".to_string(), grid_placement_json(value));
    }
    Value::Object(map)
}

fn visual_style_snapshot(style: &VisualStyle) -> Value {
    let mut map = Map::new();
    insert_color_ref(&mut map, "background", &style.background);
    if let Some(paint) = &style.background_paint {
        if let Some(value) = background_paint_json(paint) {
            map.insert("background_paint".to_string(), value);
        }
    }
    insert_color_ref(&mut map, "foreground", &style.foreground);
    insert_color_ref(&mut map, "border_color", &style.border_color);
    insert_number(&mut map, "border_width", style.border_width);
    insert_number(&mut map, "border_radius", style.border_radius);
    insert_number(
        &mut map,
        "border_top_left_radius",
        style.corner_radii.top_left,
    );
    insert_number(
        &mut map,
        "border_top_right_radius",
        style.corner_radii.top_right,
    );
    insert_number(
        &mut map,
        "border_bottom_right_radius",
        style.corner_radii.bottom_right,
    );
    insert_number(
        &mut map,
        "border_bottom_left_radius",
        style.corner_radii.bottom_left,
    );
    insert_color_ref(&mut map, "accent", &style.accent);
    insert_color_ref(&mut map, "track_color", &style.track_color);
    insert_color_ref(&mut map, "thumb_color", &style.thumb_color);
    insert_number(&mut map, "opacity", style.opacity);
    insert_number(&mut map, "background_noise", style.background_noise);
    if let Some(shadows) = &style.box_shadows {
        map.insert(
            "box_shadow".to_string(),
            Value::Array(shadows.iter().map(box_shadow_json).collect()),
        );
    }
    if let Some(transform) = style.transform {
        map.insert(
            "transform".to_string(),
            json!({
                "translate_x": transform.translate_x,
                "translate_y": transform.translate_y,
                "scale_x": transform.scale_x,
                "scale_y": transform.scale_y,
                "rotate_deg": transform.rotate_deg,
            }),
        );
    }
    Value::Object(map)
}

fn background_paint_json(paint: &BackgroundPaint) -> Option<Value> {
    match paint {
        BackgroundPaint::Color(color) => Some(json!({
            "type": "color",
            "color": color_ref_json(color),
        })),
        BackgroundPaint::Layers(layers) => Some(json!({
            "type": "layers",
            "layers": layers.iter().filter_map(background_paint_json).collect::<Vec<_>>(),
        })),
        BackgroundPaint::LinearGradient(gradient) => Some(json!({
            "type": "linear_gradient",
            "repeating": gradient.repeating,
            "angle_deg": gradient.angle_deg,
            "stops": gradient.stops.iter().map(|stop| {
                json!({
                    "color": color_ref_json(&stop.color),
                    "position": stop.position,
                })
            }).collect::<Vec<_>>(),
        })),
        BackgroundPaint::RadialGradient(gradient) => Some(json!({
            "type": "radial_gradient",
            "repeating": gradient.repeating,
            "center": gradient.center,
            "stops": gradient.stops.iter().map(|stop| {
                json!({
                    "color": color_ref_json(&stop.color),
                    "position": stop.position,
                })
            }).collect::<Vec<_>>(),
        })),
    }
}

fn box_shadow_json(shadow: &BoxShadow) -> Value {
    json!({
        "offset_x": shadow.offset_x,
        "offset_y": shadow.offset_y,
        "blur": shadow.blur,
        "spread": shadow.spread,
        "color": color_ref_json(&shadow.color),
        "inset": shadow.inset,
    })
}

fn transition_style_snapshot(style: &TransitionStyle) -> Value {
    let mut map = Map::new();
    if let Some(properties) = &style.properties {
        map.insert(
            "property".to_string(),
            Value::Array(
                properties
                    .iter()
                    .map(|property| json!(transition_property_name(*property)))
                    .collect(),
            ),
        );
    }
    if let Some(duration) = style.duration_ms {
        map.insert("duration_ms".to_string(), json!(duration));
    }
    if let Some(delay) = style.delay_ms {
        map.insert("delay_ms".to_string(), json!(delay));
    }
    if let Some(timing) = style.timing_function {
        map.insert(
            "timing_function".to_string(),
            json!(transition_timing_name(timing)),
        );
    }
    Value::Object(map)
}

fn transition_property_name(property: crate::style::TransitionProperty) -> &'static str {
    match property {
        crate::style::TransitionProperty::All => "all",
        crate::style::TransitionProperty::Background => "background",
        crate::style::TransitionProperty::Foreground => "foreground",
        crate::style::TransitionProperty::BorderColor => "border-color",
        crate::style::TransitionProperty::BorderWidth => "border-width",
        crate::style::TransitionProperty::BorderRadius => "border-radius",
        crate::style::TransitionProperty::Opacity => "opacity",
        crate::style::TransitionProperty::Color => "color",
        crate::style::TransitionProperty::Accent => "accent",
        crate::style::TransitionProperty::TrackColor => "track-color",
        crate::style::TransitionProperty::ThumbColor => "thumb-color",
        crate::style::TransitionProperty::BoxShadow => "box-shadow",
        crate::style::TransitionProperty::Transform => "transform",
    }
}

fn transition_timing_name(timing: TransitionTimingFunction) -> &'static str {
    match timing {
        TransitionTimingFunction::Linear => "linear",
        TransitionTimingFunction::Ease => "ease",
        TransitionTimingFunction::EaseIn => "ease-in",
        TransitionTimingFunction::EaseOut => "ease-out",
        TransitionTimingFunction::EaseInOut => "ease-in-out",
    }
}

fn text_style_snapshot(style: &TextStyle) -> Value {
    let mut map = Map::new();
    insert_number(&mut map, "font_size", style.font_size);
    if let Some(value) = &style.font_family {
        map.insert("font_family".to_string(), font_family_json(value));
    }
    if let Some(value) = style.font_weight {
        map.insert("font_weight".to_string(), json!(value));
    }
    insert_color_ref(&mut map, "color", &style.color);
    if let Some(value) = style.text_align {
        map.insert("text_align".to_string(), json!(text_align_name(value)));
    }
    if let Some(value) = style.text_transform {
        map.insert(
            "text_transform".to_string(),
            json!(text_transform_name(value)),
        );
    }
    if let Some(value) = style.letter_spacing {
        map.insert("letter_spacing".to_string(), text_spacing_json(value));
    }
    if let Some(value) = style.line_height {
        map.insert("line_height".to_string(), line_height_json(value));
    }
    if let Some(value) = style.font_style {
        map.insert("font_style".to_string(), json!(font_style_name(value)));
    }
    if let Some(value) = style.font_variant_numeric {
        map.insert(
            "font_variant_numeric".to_string(),
            json!(font_variant_numeric_name(value)),
        );
    }
    if let Some(value) = style.text_overflow {
        map.insert(
            "text_overflow".to_string(),
            json!(text_overflow_name(value)),
        );
    }
    Value::Object(map)
}

fn widget_style_snapshot(style: &WidgetStyle) -> Value {
    let mut map = Map::new();
    insert_number(&mut map, "table_row_height", style.table_row_height);
    insert_number(&mut map, "table_header_height", style.table_header_height);
    Value::Object(map)
}

fn part_layout_style_snapshot(style: &PartLayoutStyle) -> Value {
    let mut map = Map::new();
    insert_number(&mut map, "width", style.width);
    insert_number(&mut map, "height", style.height);
    insert_number(&mut map, "padding", style.padding);
    insert_number(&mut map, "gap", style.gap);
    Value::Object(map)
}

fn part_style_snapshot(style: &PartStyle) -> Value {
    let mut map = Map::new();
    insert_object_if_non_empty(
        &mut map,
        "layout",
        part_layout_style_snapshot(&style.layout),
    );
    insert_object_if_non_empty(&mut map, "visual", visual_style_snapshot(&style.visual));
    insert_object_if_non_empty(&mut map, "text", text_style_snapshot(&style.text));
    Value::Object(map)
}

fn insert_object_if_non_empty(map: &mut Map<String, Value>, key: &str, value: Value) {
    if value.as_object().is_some_and(|object| !object.is_empty()) {
        map.insert(key.to_string(), value);
    }
}

fn compact_part_styles_snapshot(
    style: &NodeStyle,
    matched_part_rules: Option<&std::collections::BTreeMap<String, Vec<String>>>,
) -> Value {
    let mut names = std::collections::BTreeSet::new();
    names.extend(style.parts.parts.keys().cloned());
    names.extend(style.parts.hover.keys().cloned());
    names.extend(style.parts.active.keys().cloned());
    names.extend(style.parts.focus.keys().cloned());
    names.extend(style.parts.disabled.keys().cloned());
    names.extend(style.parts.checked.keys().cloned());
    names.extend(style.parts.open.keys().cloned());
    names.extend(style.parts.expanded.keys().cloned());
    names.extend(style.parts.collapsed.keys().cloned());
    names.extend(style.parts.selected.keys().cloned());
    if let Some(matched_part_rules) = matched_part_rules {
        names.extend(matched_part_rules.keys().cloned());
    }

    let mut out = Map::new();
    for name in names {
        let mut part = Map::new();
        if let Some(matched) = matched_part_rules.and_then(|rules| rules.get(&name)) {
            if !matched.is_empty() {
                part.insert("matched_rules".to_string(), json!(matched));
            }
        }
        if let Some(base) = style.parts.parts.get(&name) {
            if let Value::Object(base_map) = part_style_snapshot(base) {
                part.extend(base_map);
            }
        }
        insert_part_state_snapshot(&mut part, "hover", style.parts.hover.get(&name));
        insert_part_state_snapshot(&mut part, "active", style.parts.active.get(&name));
        insert_part_state_snapshot(&mut part, "focus", style.parts.focus.get(&name));
        insert_part_state_snapshot(&mut part, "disabled", style.parts.disabled.get(&name));
        insert_part_state_snapshot(&mut part, "checked", style.parts.checked.get(&name));
        insert_part_state_snapshot(&mut part, "open", style.parts.open.get(&name));
        insert_part_state_snapshot(&mut part, "expanded", style.parts.expanded.get(&name));
        insert_part_state_snapshot(&mut part, "collapsed", style.parts.collapsed.get(&name));
        insert_part_state_snapshot(&mut part, "selected", style.parts.selected.get(&name));
        if !part.is_empty() {
            out.insert(name, Value::Object(part));
        }
    }
    Value::Object(out)
}

fn insert_part_state_snapshot(map: &mut Map<String, Value>, key: &str, style: Option<&PartStyle>) {
    let Some(style) = style else {
        return;
    };
    insert_object_if_non_empty(map, key, part_style_snapshot(style));
}

fn node_style_snapshot(
    style: &NodeStyle,
    matched_part_rules: Option<&std::collections::BTreeMap<String, Vec<String>>>,
) -> Value {
    let mut map = Map::new();
    map.insert("layout".to_string(), layout_style_snapshot(&style.layout));
    map.insert("visual".to_string(), visual_style_snapshot(&style.visual));
    map.insert("text".to_string(), text_style_snapshot(&style.text));
    map.insert("widget".to_string(), widget_style_snapshot(&style.widget));
    insert_object_if_non_empty(
        &mut map,
        "transition",
        transition_style_snapshot(&style.transition),
    );
    insert_object_if_non_empty(
        &mut map,
        "parts",
        compact_part_styles_snapshot(style, matched_part_rules),
    );
    map.insert("hover".to_string(), visual_style_snapshot(&style.hover));
    map.insert("active".to_string(), visual_style_snapshot(&style.active));
    map.insert("focus".to_string(), visual_style_snapshot(&style.focus));
    map.insert(
        "disabled".to_string(),
        visual_style_snapshot(&style.disabled),
    );
    map.insert("checked".to_string(), visual_style_snapshot(&style.checked));
    map.insert("open".to_string(), visual_style_snapshot(&style.open));
    map.insert(
        "expanded".to_string(),
        visual_style_snapshot(&style.expanded),
    );
    map.insert(
        "collapsed".to_string(),
        visual_style_snapshot(&style.collapsed),
    );
    map.insert(
        "selected".to_string(),
        visual_style_snapshot(&style.selected),
    );
    Value::Object(map)
}

fn computed_styles_snapshot(root: Option<&WidgetNode>, store: &StylesheetStore) -> Value {
    let Some(root) = root else {
        return json!({});
    };
    let matched_rules = matched_rule_labels_for_tree(root, store);
    let matched_part_rules = matched_part_rule_labels_for_tree(root, store);
    let mut out = Map::new();
    collect_computed_styles_snapshot(root, &matched_rules, &matched_part_rules, &mut out);
    Value::Object(out)
}

fn collect_computed_styles_snapshot(
    node: &WidgetNode,
    matched_rules: &std::collections::BTreeMap<String, Vec<String>>,
    matched_part_rules: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, Vec<String>>,
    >,
    out: &mut Map<String, Value>,
) {
    out.insert(
        node.id.clone(),
        json!({
            "matched_rules": matched_rules.get(&node.id).cloned().unwrap_or_default(),
            "style": node_style_snapshot(&node.style, matched_part_rules.get(&node.id)),
        }),
    );
    for child in &node.children {
        collect_computed_styles_snapshot(child, matched_rules, matched_part_rules, out);
    }
}

fn props_snapshot(node: &WidgetNode) -> Value {
    let props = &node.props;
    json!({
        "text": props.text.as_deref(),
        "badge": props.badge.as_deref(),
        "level": props.level.as_deref(),
        "fixed_width": props.fixed_width,
        "fixed_height": props.fixed_height,
        "disabled": props.disabled,
        "expanded": props.expanded,
        "open": props.open,
        "target": props.target.as_deref(),
        "tooltip": props.tooltip.as_deref(),
        "image_path": props.image_path.as_deref(),
        "image_fit": props.image_fit.as_deref(),
        "checked": props.checked,
        "value": props.value,
        "min": props.min,
        "max": props.max,
        "step": props.step,
        "placeholder": props.placeholder.as_deref(),
        "rows": props.rows,
        "wrap": props.wrap,
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
                    "selected_cell": table.selected.map(|(row, col)| json!({
                        "row_index": row,
                        "column_index": col,
                        "column": table.columns.get(col).map(String::as_str).unwrap_or(""),
                    })),
                    "sort": sort,
                }),
            )
        })
        .collect();
    json!({
        "checked": &state.checked,
        "expanded": &state.expanded,
        "float_val": &state.float_val,
        "float_range": &state.float_range,
        "text_val": &state.text_val,
        "text_cursor": &state.text_cursor,
        "text_scroll_y": &state.text_scroll_y,
        "container_scroll_y": &state.container_scroll_y,
        "dropdown_index": &state.dropdown_index,
        "dropdown_items_count": state.dropdown_items.iter().map(|(id, items)| (id.clone(), json!(items.len()))).collect::<Map<_, _>>(),
        "disabled": state.disabled.iter().cloned().collect::<Vec<_>>(),
        "focus_order": &state.focus_order,
        "focused": state.focused.as_deref(),
        "hovered": state.hovered.as_deref(),
        "hover_t": &state.hover_t,
        "open_t": &state.open_t,
        "selected_t": &state.selected_t,
        "pressed": state.pressed.as_deref(),
        "open_dropdown": state.open_dropdown.as_deref(),
        "dropdown_hover": state.dropdown_hover.as_ref().map(|(id, idx)| json!({"id": id, "index": idx})),
        "open_menu": state.open_menu.as_deref(),
        "open_context_menu": state.open_context_menu.as_deref(),
        "context_menu_pos": state.context_menu_pos,
        "menu_items_count": state.menu_items.iter().map(|(id, items)| (id.clone(), json!(items.len()))).collect::<Map<_, _>>(),
        "context_targets": &state.context_targets,
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
        (
            WidgetKind::Label
            | WidgetKind::Badge
            | WidgetKind::Tag
            | WidgetKind::Button
            | WidgetKind::Checkbox
            | WidgetKind::NumberInput
            | WidgetKind::ProgressBar,
            "text" | "label",
        ) => {
            target.props.text = Some(value);
            true
        }
        (
            WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal | WidgetKind::Page,
            "title",
        ) => {
            target.props.text = Some(value);
            true
        }
        (WidgetKind::Collapsible, "title" | "text") => {
            target.props.text = Some(value);
            true
        }
        (
            WidgetKind::Tab | WidgetKind::NavItem | WidgetKind::Menu | WidgetKind::MenuItem,
            "label",
        ) => {
            target.props.text = Some(value);
            true
        }
        _ => false,
    }
}

fn set_widget_level_prop(node: &mut WidgetNode, id: &str, level: String) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    if !matches!(target.kind, WidgetKind::Badge | WidgetKind::Tag) {
        return false;
    }
    target.props.level = Some(level);
    true
}

fn set_widget_open_prop(node: &mut WidgetNode, id: &str, open: bool) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    if target.kind != WidgetKind::Modal {
        return false;
    }
    target.props.open = Some(open);
    true
}

fn set_widget_expanded_prop(node: &mut WidgetNode, id: &str, expanded: bool) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    if target.kind != WidgetKind::Collapsible {
        return false;
    }
    target.props.expanded = Some(expanded);
    true
}

fn set_widget_route_value_prop(node: &mut WidgetNode, id: &str, value: String) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    if !matches!(target.kind, WidgetKind::Tabs | WidgetKind::Pages) {
        return false;
    }
    target.props.route_value = Some(value);
    true
}

fn set_widget_class_prop(node: &mut WidgetNode, id: &str, value: Option<String>) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    target.class_name = value;
    true
}

fn set_widget_badge_prop(node: &mut WidgetNode, id: &str, badge: Option<String>) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    if !matches!(
        target.kind,
        WidgetKind::Button | WidgetKind::Tab | WidgetKind::NavItem
    ) {
        return false;
    }
    target.props.badge = badge.filter(|value| !value.is_empty());
    true
}

fn format_badge_number(value: f32) -> String {
    if value.is_finite() && value.fract().abs() < f32::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn set_widget_image_prop(node: &mut WidgetNode, id: &str, prop: &str, value: String) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    if target.kind != WidgetKind::Image {
        return false;
    }
    match prop {
        "path" => target.props.image_path = (!value.is_empty()).then_some(value),
        "fit" => target.props.image_fit = Some(value.to_ascii_lowercase()),
        _ => return false,
    }
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
        if key == "parts" {
            return Dirty::Layout;
        }
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
            | "table_row_height"
            | "table-header-height"
            | "table_header_height"
            | "table-row-height"
    )
}

fn is_text_style_key(key: &str) -> bool {
    matches!(
        key,
        "foreground"
            | "color"
            | "font_size"
            | "font-size"
            | "font_family"
            | "font-family"
            | "font_weight"
            | "font-weight"
            | "text_align"
            | "text-align"
            | "text_transform"
            | "text-transform"
            | "letter_spacing"
            | "letter-spacing"
            | "line_height"
            | "line-height"
            | "font_style"
            | "font-style"
            | "font_variant_numeric"
            | "font-variant-numeric"
            | "text_overflow"
            | "text-overflow"
    )
}

fn pseudo_style_value_changes_text(key: &str, value: &Value) -> bool {
    if !matches!(
        key,
        "hover"
            | "active"
            | "focus"
            | "disabled"
            | "checked"
            | "open"
            | "expanded"
            | "collapsed"
            | "selected"
    ) {
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
            style_patch_dirty(json!({"checked": {"color": "accent"}}).as_object().unwrap()),
            Dirty::Text
        );
        assert_eq!(
            style_patch_dirty(
                json!({"parts": {"stepper": {"background": "accent"}}})
                    .as_object()
                    .unwrap()
            ),
            Dirty::Layout
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
    fn computed_style_snapshot_omits_empty_part_styles() {
        let tree = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "run",
                "type": "button",
                "props": {"text": "Run"}
            }]
        }))
        .unwrap();
        let store = StylesheetStore::default();

        let snapshot = computed_styles_snapshot(Some(&tree), &store);
        let button_style = &snapshot["run"]["style"];

        assert!(button_style["layout"].is_object());
        assert!(button_style.get("parts").is_none());
    }

    #[test]
    fn computed_style_snapshot_includes_transition_fields() {
        let mut tree = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "run",
                "type": "button",
                "props": {"text": "Run"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "Button { transition: background 180ms ease-out 25ms; }",
            )
            .unwrap();
        apply_stylesheets_to_tree(&mut tree, &mut store);

        let snapshot = computed_styles_snapshot(Some(&tree), &store);
        let transition = &snapshot["run"]["style"]["transition"];

        assert_eq!(transition["property"], json!(["background"]));
        assert_eq!(transition["duration_ms"], json!(180));
        assert_eq!(transition["delay_ms"], json!(25));
        assert_eq!(transition["timing_function"], json!("ease-out"));
    }

    #[test]
    fn clear_style_transition_state_removes_runtime_progress() {
        let mut transitions = HashMap::from([(
            "run".to_string(),
            HoverTransition {
                start: Instant::now(),
                duration: Duration::from_millis(120),
                delay: Duration::ZERO,
                from: 0.0,
                to: 1.0,
                timing: TransitionTimingFunction::EaseOut,
            },
        )]);
        let mut open_transitions = HashMap::from([(
            "mode".to_string(),
            HoverTransition {
                start: Instant::now(),
                duration: Duration::from_millis(120),
                delay: Duration::ZERO,
                from: 1.0,
                to: 0.0,
                timing: TransitionTimingFunction::EaseOut,
            },
        )]);
        let mut selected_transitions = HashMap::from([(
            "tab-a".to_string(),
            HoverTransition {
                start: Instant::now(),
                duration: Duration::from_millis(120),
                delay: Duration::ZERO,
                from: 1.0,
                to: 0.0,
                timing: TransitionTimingFunction::EaseOut,
            },
        )]);
        let mut state = WidgetState::default();
        state.hover_t.insert("run".to_string(), 0.5);
        state.open_t.insert("mode".to_string(), 0.5);
        state.selected_t.insert("tab-a".to_string(), 0.5);
        let mut state = Some(state);

        assert!(clear_style_transition_state(
            &mut transitions,
            &mut open_transitions,
            &mut selected_transitions,
            &mut state
        ));
        assert!(transitions.is_empty());
        assert!(open_transitions.is_empty());
        assert!(selected_transitions.is_empty());
        assert!(state.as_ref().unwrap().hover_t.is_empty());
        assert!(state.as_ref().unwrap().open_t.is_empty());
        assert!(state.as_ref().unwrap().selected_t.is_empty());
        assert!(!clear_style_transition_state(
            &mut transitions,
            &mut open_transitions,
            &mut selected_transitions,
            &mut state
        ));
    }

    #[test]
    fn computed_style_snapshot_includes_part_matched_rules() {
        let mut tree = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "amount",
                "type": "number_input",
                "class": "numeric"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                NumberInput::stepper { width: 34px; background: surface_alt; }
                .numeric:hover::stepper-up { color: accent; }
                "#,
            )
            .unwrap();
        apply_stylesheets_to_tree(&mut tree, &mut store);

        let snapshot = computed_styles_snapshot(Some(&tree), &store);
        let parts = &snapshot["amount"]["style"]["parts"];

        assert_eq!(
            parts["stepper"]["matched_rules"],
            json!(["user: NumberInput::stepper"])
        );
        assert_eq!(parts["stepper"]["layout"]["width"], json!(34.0));
        assert_eq!(
            parts["stepper"]["visual"]["background"]["token"],
            "surface_alt"
        );
        assert_eq!(
            parts["stepper-up"]["matched_rules"],
            json!(["user: .numeric:hover::stepper-up"])
        );
        assert_eq!(
            parts["stepper-up"]["hover"]["text"]["color"]["token"],
            "accent"
        );
        assert!(parts.get("base").is_none());
        assert!(parts.get("hover").is_none());
    }

    #[test]
    fn number_input_stepper_hit_test_uses_part_width() {
        let default_tree = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "amount",
                "type": "number_input"
            }]
        }))
        .unwrap();
        let styled_tree = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "amount",
                "type": "number_input",
                "style": {
                    "parts": {
                        "stepper": {"width": 40}
                    }
                }
            }]
        }))
        .unwrap();
        let mut widget_kinds = HashMap::new();
        collect_widget_kinds(&styled_tree, &mut widget_kinds);
        let state = WidgetState::from_tree(&styled_tree);
        let mut layout = crate::layout::LayoutResult::default();
        layout.rects.insert(
            "amount".to_string(),
            crate::layout::Rect {
                x: 100.0,
                y: 10.0,
                w: 100.0,
                h: 40.0,
            },
        );

        assert_eq!(
            number_input_step_at_pos(
                Some(&default_tree),
                &widget_kinds,
                &state,
                &layout,
                1.0,
                [165.0, 20.0],
            ),
            None
        );
        assert_eq!(
            number_input_step_at_pos(
                Some(&styled_tree),
                &widget_kinds,
                &state,
                &layout,
                1.0,
                [165.0, 20.0],
            ),
            Some(("amount".to_string(), 1.0))
        );
        assert_eq!(
            number_input_step_at_pos(
                Some(&styled_tree),
                &widget_kinds,
                &state,
                &layout,
                1.0,
                [185.0, 20.0],
            ),
            Some(("amount".to_string(), 1.0))
        );
        assert_eq!(
            number_input_step_at_pos(
                Some(&styled_tree),
                &widget_kinds,
                &state,
                &layout,
                1.0,
                [150.0, 20.0],
            ),
            None
        );
        assert_eq!(
            number_input_step_at_pos(
                Some(&styled_tree),
                &widget_kinds,
                &state,
                &layout,
                1.0,
                [135.0, 20.0],
            ),
            Some(("amount".to_string(), -1.0))
        );
        assert_eq!(
            number_input_step_at_pos(
                Some(&styled_tree),
                &widget_kinds,
                &state,
                &layout,
                1.0,
                [159.9, 20.0],
            ),
            None
        );
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
    stylesheets: StylesheetStore,
    scale_factor: f32,
    scatter: Option<ScatterWidget>,
    scatter_widget_id: Option<String>,
    scatter_decode_scratch: Vec<PointInstance>,
    primitives: Option<PrimitivesRenderer>,
    images: Option<ImageRenderer>,
    widget_tree: Option<WidgetNode>,
    widget_kinds: HashMap<String, WidgetKind>,
    caret_positions: HashMap<String, [f32; 2]>,
    resources: ResourceRegistry,
    toasts: Vec<RuntimeToast>,
    toast_overlays: Vec<ToastOverlay>,
    /// Mutable per-widget interactive state (checkbox, slider, hover, press).
    widget_state: Option<WidgetState>,
    /// Layout saved after each `apply_layout` call for hit testing.
    current_layout: Option<crate::layout::LayoutResult>,
    /// Text renderer (Label, Button labels).
    text: Option<TextRendererDg>,
    hover_transitions: HashMap<String, HoverTransition>,
    open_transitions: HashMap<String, HoverTransition>,
    open_state_snapshot: HashSet<String>,
    selected_transitions: HashMap<String, HoverTransition>,
    selected_state_snapshot: HashSet<String>,
    scatter_metrics: ScatterMetrics,
}

#[derive(Debug, Clone)]
struct HoverTransition {
    start: Instant,
    duration: Duration,
    delay: Duration,
    from: f32,
    to: f32,
    timing: TransitionTimingFunction,
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

#[derive(Debug, Clone)]
struct RuntimeToast {
    id: String,
    message: String,
    level: ToastLevel,
    duration: Option<Duration>,
    created: Instant,
    opacity: f32,
    radius: Option<f32>,
    padding: Option<f32>,
    position: ToastPosition,
}

impl RuntimeToast {
    fn expires_at(&self) -> Option<Instant> {
        self.duration
            .and_then(|duration| self.created.checked_add(duration))
    }

    fn is_expired(&self, now: Instant) -> bool {
        self.expires_at().is_some_and(|deadline| now >= deadline)
    }

    fn overlay(&self) -> ToastOverlay {
        ToastOverlay {
            id: self.id.clone(),
            message: self.message.clone(),
            level: self.level,
            opacity: self.opacity,
            radius: self.radius,
            padding: self.padding,
            position: self.position,
        }
    }
}

fn transition_config(
    style: &TransitionStyle,
) -> Option<(Duration, Duration, TransitionTimingFunction)> {
    if style.properties.as_ref().is_some_and(Vec::is_empty) {
        return None;
    }
    let duration_ms = style.duration_ms?;
    if duration_ms == 0 {
        return None;
    }
    Some((
        Duration::from_millis(duration_ms),
        Duration::from_millis(style.delay_ms.unwrap_or(0)),
        style
            .timing_function
            .unwrap_or(TransitionTimingFunction::Ease),
    ))
}

fn ease_transition(t: f32, timing: TransitionTimingFunction) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match timing {
        TransitionTimingFunction::Linear => t,
        TransitionTimingFunction::Ease => t * t * (3.0 - 2.0 * t),
        TransitionTimingFunction::EaseIn => t * t,
        TransitionTimingFunction::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        TransitionTimingFunction::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
            }
        }
    }
}

fn clear_style_transition_state(
    hover_transitions: &mut HashMap<String, HoverTransition>,
    open_transitions: &mut HashMap<String, HoverTransition>,
    selected_transitions: &mut HashMap<String, HoverTransition>,
    widget_state: &mut Option<WidgetState>,
) -> bool {
    let had_transitions = !hover_transitions.is_empty()
        || !open_transitions.is_empty()
        || !selected_transitions.is_empty();
    hover_transitions.clear();
    open_transitions.clear();
    selected_transitions.clear();
    let had_progress = widget_state.as_mut().is_some_and(|state| {
        let had_hover = !std::mem::take(&mut state.hover_t).is_empty();
        let had_open = !std::mem::take(&mut state.open_t).is_empty();
        let had_selected = !std::mem::take(&mut state.selected_t).is_empty();
        had_hover || had_open || had_selected
    });
    had_transitions || had_progress
}

fn collect_open_widget_ids(
    node: &WidgetNode,
    state: Option<&WidgetState>,
    out: &mut HashSet<String>,
) {
    if state.is_some_and(|state| state.is_open_widget(&node.id))
        || (node.kind == WidgetKind::Modal && node.props.open == Some(true))
    {
        out.insert(node.id.clone());
    }
    for child in &node.children {
        collect_open_widget_ids(child, state, out);
    }
}

fn collect_selected_widget_ids(
    node: &WidgetNode,
    state: Option<&WidgetState>,
    out: &mut HashSet<String>,
) {
    if state.is_some_and(|state| state.is_selected_widget(&node.id)) {
        out.insert(node.id.clone());
    }
    for child in &node.children {
        collect_selected_widget_ids(child, state, out);
    }
}

fn number_input_step_at_pos(
    widget_tree: Option<&WidgetNode>,
    widget_kinds: &HashMap<String, WidgetKind>,
    state: &WidgetState,
    layout: &crate::layout::LayoutResult,
    scale_factor: f32,
    pos: [f32; 2],
) -> Option<(String, f32)> {
    for (id, kind) in widget_kinds {
        if kind != &WidgetKind::NumberInput || state.is_disabled(id) {
            continue;
        }
        let Some(rect) = layout.rects.get(id) else {
            continue;
        };
        let step_w = widget_tree
            .and_then(|tree| crate::overlays::find_node(tree, id))
            .map(|node| number_stepper_width_for_style(&node.style, rect.w, scale_factor))
            .unwrap_or_else(|| number_stepper_width(rect.w, scale_factor));
        if pos[1] < rect.y || pos[1] >= rect.y + rect.h {
            continue;
        }
        if pos[0] >= rect.x && pos[0] < rect.x + step_w {
            return Some((id.clone(), -1.0));
        }
        if pos[0] >= rect.x + rect.w - step_w && pos[0] < rect.x + rect.w {
            return Some((id.clone(), 1.0));
        }
    }
    None
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

        let (scatter, upload_ms, scatter_points) = if let Some(scatter_spec) = spec.scatter {
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
            (Some(s), ms, pts)
        } else {
            (None, 0.0, Vec::new())
        };

        let primitives = spec
            .widget_tree
            .as_ref()
            .map(|_| PrimitivesRenderer::new(&device, &queue, config.format, width, height));

        let images = spec
            .widget_tree
            .as_ref()
            .map(|_| ImageRenderer::new(&device, &queue, config.format, width, height));

        let text = spec
            .widget_tree
            .as_ref()
            .map(|_| TextRendererDg::new(&device, &queue, config.format));

        let mut resources = ResourceRegistry::default();
        if let Some(tree) = &spec.widget_tree {
            resources.sync_from_tree(tree);
        }
        let widget_state = spec.widget_tree.as_ref().map(WidgetState::from_tree);
        let mut open_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_open_widget_ids(tree, widget_state.as_ref(), &mut open_state_snapshot);
        }
        let mut selected_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_selected_widget_ids(tree, widget_state.as_ref(), &mut selected_state_snapshot);
        }
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
            stylesheets: spec.stylesheets,
            scale_factor,
            scatter,
            scatter_widget_id,
            scatter_decode_scratch: scatter_points,
            primitives,
            images,
            widget_tree: spec.widget_tree,
            widget_kinds,
            caret_positions: HashMap::new(),
            resources,
            toasts: Vec::new(),
            toast_overlays: Vec::new(),
            widget_state,
            current_layout: None,
            text,
            hover_transitions: HashMap::new(),
            open_transitions: HashMap::new(),
            open_state_snapshot,
            selected_transitions: HashMap::new(),
            selected_state_snapshot,
            scatter_metrics: ScatterMetrics::default(),
        };

        state.apply_layout();

        Ok((state, upload_ms))
    }

    /// Recompute layout and push scatter viewport + primitives + text to GPU.
    fn apply_layout(&mut self) {
        self.sync_open_transitions();
        self.sync_selected_transitions();
        // Destructure to get separate borrows of each field.
        let WgpuState {
            widget_tree,
            current_layout,
            widget_state,
            resources,
            primitives,
            images,
            text,
            scatter,
            caret_positions,
            toast_overlays,
            device,
            queue,
            config,
            theme,
            scale_factor,
            stylesheets,
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
                if let Some(r) = layout.visible_rect(scatter_id) {
                    s.set_layout_rect(r.x, r.y, r.w, r.h, queue);
                }
            } else {
                s.set_layout_rect(0.0, 0.0, 0.0, 0.0, queue);
            }
        }

        if let Some(images) = images.as_mut() {
            images.update_screen_size(queue, config.width, config.height);
            images.rebuild(device, queue, tree, &layout, theme, *scale_factor);
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
                t.rebuild(
                    tree,
                    &layout,
                    theme,
                    *scale_factor,
                    state,
                    resources,
                    toast_overlays,
                    stylesheets,
                    config.width as f32,
                    config.height as f32,
                )
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
                toast_overlays,
                stylesheets,
                config.width as f32,
                config.height as f32,
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
            toast_overlays,
            stylesheets,
            config,
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
                toast_overlays,
                stylesheets,
                config.width as f32,
                config.height as f32,
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
            toast_overlays,
            stylesheets,
            config,
            ..
        } = self;

        if let (Some(tree), Some(layout), Some(state), Some(t)) = (
            widget_tree.as_ref(),
            current_layout.as_ref(),
            widget_state.as_ref(),
            text.as_mut(),
        ) {
            *caret_positions = t.rebuild(
                tree,
                layout,
                theme,
                *scale_factor,
                state,
                resources,
                toast_overlays,
                stylesheets,
                config.width as f32,
                config.height as f32,
            );
        } else {
            caret_positions.clear();
        }
    }

    /// Rebuild state-dependent primitive and text buffers without recomputing layout.
    fn rebuild_visuals(&mut self) {
        self.sync_open_transitions();
        self.sync_selected_transitions();
        self.rebuild_text();
        self.rebuild_primitives();
    }

    fn current_open_widget_ids(&self) -> HashSet<String> {
        let mut open = HashSet::new();
        if let Some(tree) = self.widget_tree.as_ref() {
            collect_open_widget_ids(tree, self.widget_state.as_ref(), &mut open);
        }
        open
    }

    fn sync_open_transitions(&mut self) -> bool {
        let current = self.current_open_widget_ids();
        if current == self.open_state_snapshot {
            return false;
        }
        let old = std::mem::replace(&mut self.open_state_snapshot, current.clone());
        let mut changed = false;
        for id in old.difference(&current) {
            changed |= self.start_open_transition(id, 0.0);
        }
        for id in current.difference(&old) {
            changed |= self.start_open_transition(id, 1.0);
        }
        changed
    }

    fn current_selected_widget_ids(&self) -> HashSet<String> {
        let mut selected = HashSet::new();
        if let Some(tree) = self.widget_tree.as_ref() {
            collect_selected_widget_ids(tree, self.widget_state.as_ref(), &mut selected);
        }
        selected
    }

    fn sync_selected_transitions(&mut self) -> bool {
        let current = self.current_selected_widget_ids();
        if current == self.selected_state_snapshot {
            return false;
        }
        let old = std::mem::replace(&mut self.selected_state_snapshot, current.clone());
        let mut changed = false;
        for id in old.difference(&current) {
            changed |= self.start_selected_transition(id, 0.0);
        }
        for id in current.difference(&old) {
            changed |= self.start_selected_transition(id, 1.0);
        }
        changed
    }

    fn update_hover_state(
        &mut self,
        new_hover: Option<String>,
        new_dropdown_hover: Option<(String, usize)>,
    ) -> bool {
        let (old_hover, old_dropdown_hover) = self
            .widget_state
            .as_ref()
            .map(|state| (state.hovered.clone(), state.dropdown_hover.clone()))
            .unwrap_or_default();
        if new_hover == old_hover && new_dropdown_hover == old_dropdown_hover {
            return false;
        }

        if let Some(id) = old_hover
            .as_deref()
            .filter(|id| Some(*id) != new_hover.as_deref())
        {
            self.start_hover_transition(id, 0.0);
        }
        if let Some(id) = new_hover
            .as_deref()
            .filter(|id| Some(*id) != old_hover.as_deref())
        {
            self.start_hover_transition(id, 1.0);
        }
        if let Some(state) = &mut self.widget_state {
            state.hovered = new_hover;
            state.dropdown_hover = new_dropdown_hover;
        }
        true
    }

    fn start_hover_transition(&mut self, id: &str, to: f32) {
        let Some((duration, delay, timing)) = self.hover_transition_config(id) else {
            self.hover_transitions.remove(id);
            if let Some(state) = &mut self.widget_state {
                state.hover_t.remove(id);
            }
            return;
        };
        let from = self
            .widget_state
            .as_ref()
            .and_then(|state| state.hover_t.get(id).copied())
            .unwrap_or(if to >= 0.5 { 0.0 } else { 1.0 });
        if let Some(state) = &mut self.widget_state {
            state.hover_t.insert(id.to_string(), from);
        }
        self.hover_transitions.insert(
            id.to_string(),
            HoverTransition {
                start: Instant::now(),
                duration,
                delay,
                from,
                to,
                timing,
            },
        );
    }

    fn start_open_transition(&mut self, id: &str, to: f32) -> bool {
        let Some((duration, delay, timing)) = self.open_transition_config(id) else {
            self.open_transitions.remove(id);
            if let Some(state) = &mut self.widget_state {
                state.open_t.remove(id);
            }
            return false;
        };
        let from = self
            .widget_state
            .as_ref()
            .and_then(|state| state.open_t.get(id).copied())
            .unwrap_or(if to >= 0.5 { 0.0 } else { 1.0 });
        if let Some(state) = &mut self.widget_state {
            state.open_t.insert(id.to_string(), from);
        }
        self.open_transitions.insert(
            id.to_string(),
            HoverTransition {
                start: Instant::now(),
                duration,
                delay,
                from,
                to,
                timing,
            },
        );
        true
    }

    fn start_selected_transition(&mut self, id: &str, to: f32) -> bool {
        let Some((duration, delay, timing)) = self.selected_transition_config(id) else {
            self.selected_transitions.remove(id);
            if let Some(state) = &mut self.widget_state {
                state.selected_t.remove(id);
            }
            return false;
        };
        let from = self
            .widget_state
            .as_ref()
            .and_then(|state| state.selected_t.get(id).copied())
            .unwrap_or(if to >= 0.5 { 0.0 } else { 1.0 });
        if let Some(state) = &mut self.widget_state {
            state.selected_t.insert(id.to_string(), from);
        }
        self.selected_transitions.insert(
            id.to_string(),
            HoverTransition {
                start: Instant::now(),
                duration,
                delay,
                from,
                to,
                timing,
            },
        );
        true
    }

    fn hover_transition_config(
        &self,
        id: &str,
    ) -> Option<(Duration, Duration, TransitionTimingFunction)> {
        let tree = self.widget_tree.as_ref()?;
        let node = find_widget(tree, id)?;
        transition_config(&node.style.transition)
    }

    fn open_transition_config(
        &self,
        id: &str,
    ) -> Option<(Duration, Duration, TransitionTimingFunction)> {
        let tree = self.widget_tree.as_ref()?;
        let node = find_widget(tree, id)?;
        transition_config(&node.style.transition)
    }

    fn selected_transition_config(
        &self,
        id: &str,
    ) -> Option<(Duration, Duration, TransitionTimingFunction)> {
        let tree = self.widget_tree.as_ref()?;
        let node = find_widget(tree, id)?;
        transition_config(&node.style.transition)
    }

    fn tick_hover_transitions(&mut self) -> bool {
        if self.hover_transitions.is_empty() {
            return false;
        }
        let now = Instant::now();
        let ids: Vec<String> = self.hover_transitions.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut changed = false;
        for id in ids {
            let Some(transition) = self.hover_transitions.get(&id) else {
                continue;
            };
            let elapsed = now.saturating_duration_since(transition.start);
            let raw_t = if elapsed < transition.delay {
                0.0
            } else {
                let active = elapsed - transition.delay;
                (active.as_secs_f32() / transition.duration.as_secs_f32()).clamp(0.0, 1.0)
            };
            let eased = ease_transition(raw_t, transition.timing);
            let value = transition.from + (transition.to - transition.from) * eased;
            if let Some(state) = &mut self.widget_state {
                state.hover_t.insert(id.clone(), value.clamp(0.0, 1.0));
            }
            changed = true;
            if raw_t >= 1.0 {
                finished.push(id);
            }
        }
        for id in finished {
            let Some(transition) = self.hover_transitions.remove(&id) else {
                continue;
            };
            if let Some(state) = &mut self.widget_state {
                state.hover_t.remove(&id);
                if transition.to > 0.0 {
                    state.hover_t.insert(id, 1.0);
                }
            }
        }
        changed
    }

    fn tick_open_transitions(&mut self) -> bool {
        if self.open_transitions.is_empty() {
            return false;
        }
        let now = Instant::now();
        let ids: Vec<String> = self.open_transitions.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut changed = false;
        for id in ids {
            let Some(transition) = self.open_transitions.get(&id) else {
                continue;
            };
            let elapsed = now.saturating_duration_since(transition.start);
            let raw_t = if elapsed < transition.delay {
                0.0
            } else {
                let active = elapsed - transition.delay;
                (active.as_secs_f32() / transition.duration.as_secs_f32()).clamp(0.0, 1.0)
            };
            let eased = ease_transition(raw_t, transition.timing);
            let value = transition.from + (transition.to - transition.from) * eased;
            if let Some(state) = &mut self.widget_state {
                state.open_t.insert(id.clone(), value.clamp(0.0, 1.0));
            }
            changed = true;
            if raw_t >= 1.0 {
                finished.push(id);
            }
        }
        for id in finished {
            let Some(transition) = self.open_transitions.remove(&id) else {
                continue;
            };
            if let Some(state) = &mut self.widget_state {
                state.open_t.remove(&id);
                if transition.to > 0.0 {
                    state.open_t.insert(id, 1.0);
                }
            }
        }
        changed
    }

    fn tick_selected_transitions(&mut self) -> bool {
        if self.selected_transitions.is_empty() {
            return false;
        }
        let now = Instant::now();
        let ids: Vec<String> = self.selected_transitions.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut changed = false;
        for id in ids {
            let Some(transition) = self.selected_transitions.get(&id) else {
                continue;
            };
            let elapsed = now.saturating_duration_since(transition.start);
            let raw_t = if elapsed < transition.delay {
                0.0
            } else {
                let active = elapsed - transition.delay;
                (active.as_secs_f32() / transition.duration.as_secs_f32()).clamp(0.0, 1.0)
            };
            let eased = ease_transition(raw_t, transition.timing);
            let value = transition.from + (transition.to - transition.from) * eased;
            if let Some(state) = &mut self.widget_state {
                state.selected_t.insert(id.clone(), value.clamp(0.0, 1.0));
            }
            changed = true;
            if raw_t >= 1.0 {
                finished.push(id);
            }
        }
        for id in finished {
            let Some(transition) = self.selected_transitions.remove(&id) else {
                continue;
            };
            if let Some(state) = &mut self.widget_state {
                state.selected_t.remove(&id);
                if transition.to > 0.0 {
                    state.selected_t.insert(id, 1.0);
                }
            }
        }
        changed
    }

    fn cancel_hover_transitions(&mut self) -> bool {
        clear_style_transition_state(
            &mut self.hover_transitions,
            &mut self.open_transitions,
            &mut self.selected_transitions,
            &mut self.widget_state,
        )
    }

    fn has_style_transitions(&self) -> bool {
        !self.hover_transitions.is_empty()
            || !self.open_transitions.is_empty()
            || !self.selected_transitions.is_empty()
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

    fn scatter_pick_payload(&self, pos: [f32; 2]) -> Option<(String, String)> {
        let id = self.scatter_widget_id.as_ref()?.clone();
        let (index, point) =
            self.scatter
                .as_ref()?
                .pick_point(&self.scatter_decode_scratch, pos[0], pos[1], 8.0)?;
        Some((
            id,
            json!({
                "index": index,
                "x": point.position[0],
                "y": point.position[1],
                "z": point.position[2],
            })
            .to_string(),
        ))
    }

    /// Hit test interactive UI widgets at physical pixel position `pos`.
    fn hit_test_ui(&self, pos: [f32; 2]) -> Option<(String, WidgetKind)> {
        let (tree, layout) = match (self.widget_tree.as_ref(), self.current_layout.as_ref()) {
            (Some(t), Some(l)) => (t, l),
            _ => return None,
        };
        let state = self.widget_state.as_ref()?;
        hit_test(tree, layout, pos)
            .filter(|(id, kind)| {
                kind != &WidgetKind::Collapsible || self.collapsible_header_contains(id, pos)
            })
            .filter(|(id, _)| !state.is_disabled(id))
    }

    fn hit_test_hover(&self, pos: [f32; 2]) -> Option<(String, WidgetKind)> {
        if let Some(id) = self.menu_item_at(pos) {
            return Some((id, WidgetKind::MenuItem));
        }
        if self.menu_popup_contains(pos) {
            return None;
        }
        let (tree, layout) = match (self.widget_tree.as_ref(), self.current_layout.as_ref()) {
            (Some(t), Some(l)) => (t, l),
            _ => return None,
        };
        let state = self.widget_state.as_ref()?;
        hit_test_hover(tree, layout, pos)
            .filter(|(id, kind)| {
                kind != &WidgetKind::Collapsible || self.collapsible_header_contains(id, pos)
            })
            .filter(|(id, _)| !state.is_disabled(id))
    }

    fn collapsible_header_contains(&self, id: &str, pos: [f32; 2]) -> bool {
        let Some(tree) = self.widget_tree.as_ref() else {
            return false;
        };
        let Some(layout) = self.current_layout.as_ref() else {
            return false;
        };
        let Some(node) = find_widget(tree, id) else {
            return false;
        };
        let Some(rect) = layout.rects.get(id) else {
            return false;
        };
        if node.kind != WidgetKind::Collapsible {
            return false;
        }
        let header_h =
            collapsible_header_height_for_style(&node.style, &self.theme, self.scale_factor)
                .min(rect.h.max(0.0));
        pos[0] >= rect.x
            && pos[0] < rect.x + rect.w
            && pos[1] >= rect.y
            && pos[1] < rect.y + header_h
    }

    fn modal_blocks_point(&self, pos: [f32; 2]) -> bool {
        let (tree, layout) = match (self.widget_tree.as_ref(), self.current_layout.as_ref()) {
            (Some(t), Some(l)) => (t, l),
            _ => return false,
        };
        modal_blocks_point(tree, layout, pos)
    }

    fn has_active_modal(&self) -> bool {
        self.widget_tree
            .as_ref()
            .is_some_and(|tree| has_active_modal(tree))
    }

    fn close_active_modal(&mut self) -> Option<String> {
        let tree = self.widget_tree.as_mut()?;
        let closed = close_active_modal(tree)?;
        if let Some(state) = &mut self.widget_state {
            state.focus_widget(None);
            state.pressed = None;
            state.close_popups();
        }
        self.apply_layout();
        Some(closed)
    }

    fn number_input_step_at(&self, pos: [f32; 2]) -> Option<(String, f32)> {
        number_input_step_at_pos(
            self.widget_tree.as_ref(),
            &self.widget_kinds,
            self.widget_state.as_ref()?,
            self.current_layout.as_ref()?,
            self.scale_factor,
            pos,
        )
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
                    self.reapply_stylesheets();
                    return Some(Dirty::Full);
                }
            }
            return None;
        }
        if let CommandValue::Text(text) = &value {
            if matches!(
                kind,
                WidgetKind::Label
                    | WidgetKind::Badge
                    | WidgetKind::Tag
                    | WidgetKind::Button
                    | WidgetKind::Panel
                    | WidgetKind::Sidebar
                    | WidgetKind::Checkbox
                    | WidgetKind::NumberInput
                    | WidgetKind::ProgressBar
                    | WidgetKind::Collapsible
                    | WidgetKind::Modal
                    | WidgetKind::Page
                    | WidgetKind::Tab
                    | WidgetKind::NavItem
                    | WidgetKind::Menu
                    | WidgetKind::MenuItem
            ) {
                if let Some(tree) = self.widget_tree.as_mut() {
                    if set_widget_text_prop(tree, id, prop, text.clone()) {
                        return Some(Dirty::Text);
                    }
                }
            }
        }
        if matches!(kind, WidgetKind::Badge | WidgetKind::Tag) && prop == "level" {
            let CommandValue::Text(level) = value else {
                eprintln!(
                    "DragonGUI: ignoring unsupported live SetProp for widget {id:?} ({kind:?}).{prop}"
                );
                return None;
            };
            if let Some(tree) = self.widget_tree.as_mut() {
                if set_widget_level_prop(tree, id, level) {
                    self.reapply_stylesheets();
                    return Some(Dirty::Full);
                }
            }
            return None;
        }
        if kind == WidgetKind::Modal && prop == "open" {
            let CommandValue::Bool(open) = value else {
                eprintln!(
                    "DragonGUI: ignoring unsupported live SetProp for widget {id:?} ({kind:?}).{prop}"
                );
                return None;
            };
            if let Some(tree) = self.widget_tree.as_mut() {
                if set_widget_open_prop(tree, id, open) {
                    return Some(Dirty::Layout);
                }
            }
            return None;
        }
        if kind == WidgetKind::Collapsible && prop == "expanded" {
            let CommandValue::Bool(expanded) = value else {
                eprintln!(
                    "DragonGUI: ignoring unsupported live SetProp for widget {id:?} ({kind:?}).{prop}"
                );
                return None;
            };
            if let Some(state) = self.widget_state.as_mut() {
                state.set_expanded(id, expanded)?;
            }
            if let Some(tree) = self.widget_tree.as_mut() {
                set_widget_expanded_prop(tree, id, expanded);
            }
            return Some(Dirty::Layout);
        }
        if matches!(
            kind,
            WidgetKind::Button | WidgetKind::Tab | WidgetKind::NavItem
        ) && prop == "badge"
        {
            let badge = match value {
                CommandValue::Text(text) => Some(text),
                CommandValue::Float(value) => Some(format_badge_number(value)),
                CommandValue::None => None,
                other => {
                    eprintln!(
                        "DragonGUI: ignoring unsupported live badge value for widget {id:?}: {other:?}"
                    );
                    return None;
                }
            };
            if let Some(tree) = self.widget_tree.as_mut() {
                if set_widget_badge_prop(tree, id, badge) {
                    return Some(Dirty::Text);
                }
            }
            return None;
        }
        if kind == WidgetKind::Image && matches!(prop, "path" | "fit") {
            let CommandValue::Text(text) = value else {
                eprintln!(
                    "DragonGUI: ignoring unsupported live SetProp for widget {id:?} ({kind:?}).{prop}"
                );
                return None;
            };
            if prop == "path" {
                if let Some(images) = self.images.as_mut() {
                    images.forget_path(&text);
                }
            }
            if let Some(tree) = self.widget_tree.as_mut() {
                if set_widget_image_prop(tree, id, prop, text) {
                    return Some(Dirty::Full);
                }
            }
            return None;
        }
        let state = self.widget_state.as_mut()?;
        if matches!(kind, WidgetKind::Tabs | WidgetKind::Pages) && prop == "value" {
            let CommandValue::Text(value) = value else {
                eprintln!(
                    "DragonGUI: ignoring unsupported live SetProp for widget {id:?} ({kind:?}).{prop}"
                );
                return None;
            };
            let changed = match kind {
                WidgetKind::Tabs => state.set_active_tab_value(id, &value).is_some(),
                WidgetKind::Pages => state.set_active_page_value(id, &value).is_some(),
                _ => false,
            };
            if !changed {
                eprintln!(
                    "DragonGUI: ignoring live SetProp for widget {id:?} ({kind:?}).{prop}: unknown or disabled route {value:?}"
                );
                return None;
            }
            if let Some(tree) = self.widget_tree.as_mut() {
                set_widget_route_value_prop(tree, id, value);
            }
            return Some(Dirty::Layout);
        }
        match (kind, prop, value) {
            (WidgetKind::Checkbox, "checked", CommandValue::Bool(v)) => {
                state.set_checked(id, v)?;
                Some(Dirty::Visual)
            }
            (WidgetKind::Slider, "value", CommandValue::Float(v)) => {
                state.try_set_float(id, v)?;
                Some(Dirty::Visual)
            }
            (WidgetKind::NumberInput, "value", CommandValue::Float(v)) => {
                state.set_number_value(id, v)?;
                Some(Dirty::Text)
            }
            (WidgetKind::ProgressBar, "value", CommandValue::Float(v)) => {
                state.try_set_float(id, v)?;
                Some(Dirty::Visual)
            }
            (WidgetKind::Dropdown, "value", CommandValue::Text(v)) => {
                state.set_dropdown_value(id, &v)?;
                Some(Dirty::Text)
            }
            (WidgetKind::TextInput | WidgetKind::TextArea, "value", CommandValue::Text(v)) => {
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
        node.inline_style =
            crate::style::NodeStyle::from_json(Some(&Value::Object(node.style_json.clone())));
        let dirty = style_patch_dirty(patch);
        self.reapply_stylesheets();
        Ok(Some(dirty))
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

    fn refresh_table_sort(&mut self, id: &str) {
        let Some((resource_id, sort, rows)) = self
            .widget_state
            .as_ref()
            .and_then(|state| state.table(id))
            .map(|table| (table.resource_id.clone(), table.sort, table.rows))
        else {
            return;
        };
        let row_order = sort.and_then(|(col, direction)| {
            self.resources
                .sorted_table_rows(resource_id.as_deref(), col, rows, direction)
        });
        if let Some(state) = &mut self.widget_state {
            state.set_table_row_order(id, row_order);
        }
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
        if matches!(dirty, Dirty::Layout | Dirty::Full) {
            self.cancel_hover_transitions();
        }
        match dirty {
            Dirty::Layout | Dirty::Full => self.apply_layout(),
            Dirty::Text => self.rebuild_visuals(),
            Dirty::Visual => self.rebuild_primitives(),
            Dirty::GpuData => {}
        }
    }

    fn reapply_stylesheets(&mut self) {
        self.cancel_hover_transitions();
        if let Some(tree) = &mut self.widget_tree {
            apply_stylesheets_to_tree(tree, &mut self.stylesheets);
        }
    }

    fn set_stylesheet(&mut self, origin: StylesheetOrigin, css: &str) -> Result<(), String> {
        self.stylesheets
            .set_stylesheet(origin, css)
            .map_err(|error| error.to_string())?;
        self.reapply_stylesheets();
        self.apply_layout();
        Ok(())
    }

    fn clear_stylesheets(&mut self, origin: StylesheetOrigin) {
        self.stylesheets.clear(origin);
        self.reapply_stylesheets();
        self.apply_layout();
    }

    fn show_toast(
        &mut self,
        id: String,
        message: String,
        level: ToastLevel,
        duration_ms: Option<u64>,
        opacity: Option<f32>,
        radius: Option<f32>,
        padding: Option<f32>,
        position: Option<ToastPosition>,
    ) {
        let existing_style = self.toasts.iter().find(|existing| existing.id == id);
        let toast = RuntimeToast {
            id,
            message,
            level,
            duration: duration_ms.map(Duration::from_millis),
            created: Instant::now(),
            opacity: opacity
                .or_else(|| existing_style.map(|toast| toast.opacity))
                .unwrap_or(1.0)
                .clamp(0.0, 1.0),
            radius: radius.or_else(|| existing_style.and_then(|toast| toast.radius)),
            padding: padding.or_else(|| existing_style.and_then(|toast| toast.padding)),
            position: position
                .or_else(|| existing_style.map(|toast| toast.position))
                .unwrap_or_default(),
        };
        if let Some(existing) = self
            .toasts
            .iter_mut()
            .find(|existing| existing.id == toast.id)
        {
            *existing = toast;
        } else {
            self.toasts.push(toast);
        }
        self.refresh_toast_overlays();
    }

    fn dismiss_toast(&mut self, id: &str) -> bool {
        let before = self.toasts.len();
        self.toasts.retain(|toast| toast.id != id);
        let changed = self.toasts.len() != before;
        if changed {
            self.refresh_toast_overlays();
        }
        changed
    }

    fn expire_toasts(&mut self) -> bool {
        let now = Instant::now();
        let before = self.toasts.len();
        self.toasts.retain(|toast| !toast.is_expired(now));
        let changed = self.toasts.len() != before;
        if changed {
            self.refresh_toast_overlays();
        }
        changed
    }

    fn next_toast_deadline(&self) -> Option<Instant> {
        self.toasts
            .iter()
            .filter_map(RuntimeToast::expires_at)
            .min()
    }

    fn refresh_toast_overlays(&mut self) {
        self.toast_overlays = self.toasts.iter().map(RuntimeToast::overlay).collect();
    }

    fn toast_snapshot(&self) -> Value {
        let now = Instant::now();
        json!({
            "count": self.toasts.len(),
            "items": self.toasts.iter().map(|toast| {
                json!({
                    "id": toast.id.as_str(),
                    "message": toast.message.as_str(),
                    "level": toast.level.as_str(),
                    "duration_ms": toast.duration.map(|duration| duration.as_millis() as u64),
                    "age_ms": now.saturating_duration_since(toast.created).as_millis() as u64,
                    "opacity": toast.opacity,
                    "radius": toast.radius,
                    "padding": toast.padding,
                    "position": toast.position.as_str(),
                })
            }).collect::<Vec<_>>(),
        })
    }

    fn debug_snapshot_value(&self) -> Value {
        json!({
            "window": {
                "width": self.config.width,
                "height": self.config.height,
                "scale_factor": self.scale_factor,
            },
            "theme": theme_snapshot(&self.theme),
            "stylesheets": {
                "framework_rules": self.stylesheets.rules(crate::css_style::StylesheetOrigin::Framework).len(),
                "theme_rules": self.stylesheets.rules(crate::css_style::StylesheetOrigin::Theme).len(),
                "user_rules": self.stylesheets.rules(crate::css_style::StylesheetOrigin::User).len(),
                "warning_count": self.stylesheets.warnings().len(),
                "last_error": self.stylesheets.last_error.as_deref(),
            },
            "computed_styles": computed_styles_snapshot(self.widget_tree.as_ref(), &self.stylesheets),
            "tree": self.widget_tree.as_ref().map(node_snapshot),
            "layout": layout_snapshot(self.current_layout.as_ref()),
            "state": widget_state_snapshot(self.widget_state.as_ref()),
            "toasts": self.toast_snapshot(),
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

    fn menu_item_at(&self, pos: [f32; 2]) -> Option<String> {
        let state = self.widget_state.as_ref()?;
        let menu_id = state
            .open_menu
            .as_deref()
            .or(state.open_context_menu.as_deref())?;
        let rect = self.menu_popup_rect(menu_id)?;
        if !rect_contains_pos(&rect, pos) {
            return None;
        }
        let row_h = self.theme.control_height() * self.scale_factor;
        let idx = ((pos[1] - rect.y) / row_h).floor() as usize;
        let item = state.menu_items.get(menu_id)?.get(idx)?;
        if item.disabled || state.is_disabled(&item.id) {
            None
        } else {
            Some(item.id.clone())
        }
    }

    fn menu_popup_contains(&self, pos: [f32; 2]) -> bool {
        let Some(state) = self.widget_state.as_ref() else {
            return false;
        };
        state
            .open_menu
            .as_deref()
            .or(state.open_context_menu.as_deref())
            .and_then(|id| self.menu_popup_rect(id))
            .is_some_and(|rect| rect_contains_pos(&rect, pos))
    }

    fn has_open_menu_popup(&self) -> bool {
        self.widget_state
            .as_ref()
            .is_some_and(|state| state.open_menu.is_some() || state.open_context_menu.is_some())
    }

    fn close_popups(&mut self) -> bool {
        let Some(state) = self.widget_state.as_mut() else {
            return false;
        };
        let had_popup = state.open_dropdown.is_some()
            || state.open_menu.is_some()
            || state.open_context_menu.is_some();
        state.close_popups();
        if had_popup {
            self.rebuild_visuals();
        }
        had_popup
    }

    fn open_context_menu_at(&mut self, menu_id: &str, pos: [f32; 2]) -> bool {
        let opened = self
            .widget_state
            .as_mut()
            .map(|state| state.open_context_menu(menu_id, pos))
            .unwrap_or(false);
        if opened {
            self.rebuild_visuals();
        }
        opened
    }

    fn context_menu_for_pos(&self, pos: [f32; 2]) -> Option<String> {
        let (target_id, _kind) = self.hit_test_ui(pos)?;
        let state = self.widget_state.as_ref()?;
        state.context_targets.get(&target_id).cloned()
    }

    fn menu_popup_rect(&self, id: &str) -> Option<crate::layout::Rect> {
        let tree = self.widget_tree.as_ref()?;
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        let items = state.menu_items.get(id)?;
        if items.is_empty() {
            return None;
        }
        let node = find_widget(tree, id)?;
        let row_h = self.theme.control_height() * self.scale_factor;
        let root = layout
            .rects
            .get(&tree.id)
            .copied()
            .unwrap_or(crate::layout::Rect {
                x: 0.0,
                y: 0.0,
                w: self.config.width as f32,
                h: self.config.height as f32,
            });
        let mut width = menu_popup_width(
            items,
            node.props.fixed_width,
            &self.theme,
            self.scale_factor,
        );
        let height = row_h * items.len() as f32;
        let (mut x, mut y) = if node.kind == WidgetKind::Menu {
            let r = layout.rects.get(id)?;
            width = width.max(r.w);
            (r.x, r.y + r.h)
        } else {
            let pos = state.context_menu_pos?;
            (pos[0], pos[1])
        };
        x = x.clamp(root.x, (root.x + root.w - width).max(root.x));
        y = y.clamp(root.y, (root.y + root.h - height).max(root.y));
        Some(crate::layout::Rect {
            x,
            y,
            w: width,
            h: height,
        })
    }

    fn table_at(&self, pos: [f32; 2]) -> Option<String> {
        self.hit_test_ui(pos)
            .and_then(|(id, kind)| (kind == WidgetKind::DataFrameTable).then_some(id))
    }

    fn text_area_at(&self, pos: [f32; 2]) -> Option<String> {
        self.hit_test_ui(pos)
            .and_then(|(id, kind)| (kind == WidgetKind::TextArea).then_some(id))
    }

    fn scroll_container_at(&self, pos: [f32; 2]) -> Option<String> {
        let tree = self.widget_tree.as_ref()?;
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        scroll_container_at_pos(tree, layout, state, pos)
    }

    fn text_area_scroll_geometry(&self, id: &str) -> Option<(f32, f32)> {
        let tree = self.widget_tree.as_ref()?;
        let layout = self.current_layout.as_ref()?;
        let node = crate::overlays::find_node(tree, id)?;
        let rect = layout
            .visible_rect(id)
            .or_else(|| layout.rects.get(id).copied())?;
        let pad = self.theme.spacing * self.scale_factor;
        let visible_h = (rect.h - pad * 2.0).max(1.0);
        let font_size = crate::text::text_font_size(node, &self.theme, self.scale_factor);
        let line_h = crate::text::text_line_height(font_size, &self.theme, self.scale_factor);
        Some((visible_h, line_h))
    }

    fn scroll_text_area(&mut self, id: &str, wheel_y: f32) -> bool {
        let Some((visible_h, line_h)) = self.text_area_scroll_geometry(id) else {
            return false;
        };
        let changed = self
            .widget_state
            .as_mut()
            .map(|state| state.scroll_text_area(id, -wheel_y * line_h * 3.0, visible_h, line_h))
            .unwrap_or(false);
        if changed {
            self.rebuild_visuals();
        }
        changed
    }

    fn scroll_container(&mut self, id: &str, wheel_y: f32) -> bool {
        let Some(tree) = self.widget_tree.as_ref() else {
            return false;
        };
        let Some(layout) = self.current_layout.as_ref() else {
            return false;
        };
        let Some(node) = find_widget(tree, id) else {
            return false;
        };
        let max_scroll = layout
            .scroll_max_y
            .get(id)
            .copied()
            .unwrap_or_else(|| scroll_container_max_y(node, layout));
        if max_scroll <= 0.0 {
            return false;
        }
        let delta_y = -wheel_y * self.theme.control_height() * self.scale_factor * 0.75;
        let changed = self
            .widget_state
            .as_mut()
            .map(|state| state.scroll_container(id, delta_y, max_scroll))
            .unwrap_or(false);
        if changed {
            self.apply_layout();
        }
        changed
    }

    fn ensure_text_area_cursor_visible(&mut self, id: &str) -> bool {
        let Some((visible_h, line_h)) = self.text_area_scroll_geometry(id) else {
            return false;
        };
        self.widget_state
            .as_mut()
            .map(|state| state.ensure_text_area_cursor_visible(id, visible_h, line_h))
            .unwrap_or(false)
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
        let metrics = self
            .widget_tree
            .as_ref()
            .and_then(|root| crate::overlays::find_node(root, &id))
            .map(|node| table::metrics_for_node(node, &self.theme, self.scale_factor))
            .unwrap_or_else(|| table::metrics(&self.theme, self.scale_factor));
        table::hit(table_state, rect, metrics, pos).map(|hit| (id, hit))
    }

    fn table_selection_payload(&self, id: &str, row: usize, col: usize) -> Option<String> {
        let table_state = self.widget_state.as_ref()?.table(id)?;
        let column = table_state.columns.get(col)?.clone();
        let source_row = table::source_row(table_state, row);
        let value = table::cell_text(table_state, &self.resources, row, col);
        Some(
            json!({
                "row_index": source_row,
                "column_index": col,
                "column": column,
                "value": value,
            })
            .to_string(),
        )
    }

    fn table_visible_counts(&self, id: &str) -> Option<(usize, usize)> {
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        let table_state = state.table(id)?;
        let rect = layout.rects.get(id)?;
        let metrics = self
            .widget_tree
            .as_ref()
            .and_then(|root| crate::overlays::find_node(root, id))
            .map(|node| table::metrics_for_node(node, &self.theme, self.scale_factor))
            .unwrap_or_else(|| table::metrics(&self.theme, self.scale_factor));
        let visible = table::visible(table_state, rect, metrics);
        Some((visible.row_count, visible.col_count))
    }

    fn focus_widget(&mut self, id: Option<String>) {
        let focused = id.clone();
        if let Some(ws) = &mut self.widget_state {
            ws.focus_widget(id);
        }
        if let Some(id) = focused.as_deref() {
            if self.widget_kind(id) == Some(WidgetKind::TextArea) {
                self.ensure_text_area_cursor_visible(id);
            }
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
        if !matches!(
            kind,
            WidgetKind::TextInput | WidgetKind::TextArea | WidgetKind::NumberInput
        ) {
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
            if let Some(images) = &self.images {
                images.render(&mut pass);
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

fn rect_contains_pos(r: &crate::layout::Rect, pos: [f32; 2]) -> bool {
    pos[0] >= r.x && pos[0] < r.x + r.w && pos[1] >= r.y && pos[1] < r.y + r.h
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
    scatter_press_pos: Option<[f32; 2]>,
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
            scatter_press_pos: None,
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
                            gpu.reapply_stylesheets();
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
                            gpu.reapply_stylesheets();
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
            Command::SetStylesheet { origin, css } => {
                let detail = Some(format!("origin={origin:?}, css_bytes={}", css.len()));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "SetStylesheet",
                            None,
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match gpu.set_stylesheet(origin, &css) {
                        Ok(()) => (Some(Dirty::Full), "applied".to_string(), true),
                        Err(err) => {
                            eprintln!("DragonGUI: failed to apply stylesheet: {err}");
                            (None, format!("error: {err}"), false)
                        }
                    }
                };
                self.record_runtime_command("SetStylesheet", None, detail, dirty, &outcome, redraw)
            }
            Command::ClearStylesheets { origin } => {
                let detail = Some(format!("origin={origin:?}"));
                let (dirty, outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "ClearStylesheets",
                            None,
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    gpu.clear_stylesheets(origin);
                    (Some(Dirty::Full), "applied".to_string(), true)
                };
                self.record_runtime_command(
                    "ClearStylesheets",
                    None,
                    detail,
                    dirty,
                    &outcome,
                    redraw,
                )
            }
            Command::ShowToast {
                id,
                message,
                level,
                duration_ms,
                opacity,
                radius,
                padding,
                position,
            } => {
                let detail = Some(format!(
                    "level={level}, duration_ms={}, position={}",
                    duration_ms
                        .map(|duration| duration.to_string())
                        .unwrap_or_else(|| "persistent".to_string()),
                    position.as_deref().unwrap_or("default")
                ));
                let (outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "ShowToast",
                            Some(id),
                            detail,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    match ToastLevel::from_str(&level) {
                        Some(level) => {
                            let position = match position.as_deref() {
                                Some(value) => match ToastPosition::from_str(value) {
                                    Some(position) => Some(position),
                                    None => {
                                        return self.record_runtime_command(
                                            "ShowToast",
                                            Some(id),
                                            detail,
                                            None,
                                            &format!("unknown_position: {value}"),
                                            false,
                                        )
                                    }
                                },
                                None => None,
                            };
                            gpu.show_toast(
                                id.clone(),
                                message,
                                level,
                                duration_ms,
                                opacity,
                                radius,
                                padding,
                                position,
                            );
                            gpu.rebuild_visuals();
                            ("applied".to_string(), true)
                        }
                        None => (format!("unknown_level: {level}"), false),
                    }
                };
                self.record_runtime_command("ShowToast", Some(id), detail, None, &outcome, redraw)
            }
            Command::DismissToast { id } => {
                let (outcome, redraw) = {
                    let Some(gpu) = &mut self.gpu else {
                        return self.record_runtime_command(
                            "DismissToast",
                            Some(id),
                            None,
                            None,
                            "gpu_not_ready",
                            false,
                        );
                    };
                    if gpu.dismiss_toast(&id) {
                        gpu.rebuild_visuals();
                        ("dismissed".to_string(), true)
                    } else {
                        ("missing".to_string(), false)
                    }
                };
                self.record_runtime_command("DismissToast", Some(id), None, None, &outcome, redraw)
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
        let needs_text_rebuild = matches!(kind, WidgetKind::Dropdown | WidgetKind::Menu);
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
            WidgetKind::Collapsible => {
                let new_val = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.toggle_expanded(id));
                if let Some(expanded) = new_val {
                    if let Some(gpu) = &mut self.gpu {
                        if let Some(tree) = gpu.widget_tree.as_mut() {
                            set_widget_expanded_prop(tree, id, expanded);
                        }
                    }
                    self.emit_change(id, ChangeValue::Bool(expanded));
                    needs_layout_rebuild = true;
                }
            }
            WidgetKind::Dropdown => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.toggle_dropdown(id);
                    }
                }
            }
            WidgetKind::Menu => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.toggle_menu(id);
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
                // Activation happens while hover/tooltip state may still be visible.
                // Keep the text layer synchronized with primitive state changes.
                gpu.rebuild_visuals();
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
        let mut payload = None;
        if let Some(gpu) = &mut self.gpu {
            if let Some(ws) = &mut gpu.widget_state {
                ws.select_table_cell(id, row, col);
            }
            payload = gpu.table_selection_payload(id, row, col);
            gpu.rebuild_visuals();
        }
        if let Some(payload) = payload {
            self.emit_change(id, ChangeValue::Text(payload));
        }
        self.request_redraw();
    }

    fn emit_current_table_selection(&mut self, id: &str) -> bool {
        let payload = self.gpu.as_ref().and_then(|gpu| {
            gpu.widget_state
                .as_ref()
                .and_then(|ws| ws.current_table_selection(id))
                .and_then(|(row, col)| gpu.table_selection_payload(id, row, col))
        });
        if let Some(payload) = payload {
            self.emit_change(id, ChangeValue::Text(payload));
            true
        } else {
            false
        }
    }

    fn move_table_selection(
        &mut self,
        id: &str,
        row_delta: isize,
        col_delta: isize,
        visible_rows: usize,
        visible_cols: usize,
    ) {
        let mut changed = false;
        if let Some(gpu) = &mut self.gpu {
            changed = gpu
                .widget_state
                .as_mut()
                .map(|ws| {
                    ws.move_table_selection(
                        id,
                        row_delta,
                        col_delta,
                        visible_rows.max(1),
                        visible_cols.max(1),
                    )
                })
                .unwrap_or(false);
            if changed {
                gpu.rebuild_visuals();
            }
        }
        if changed {
            self.emit_current_table_selection(id);
            self.request_redraw();
        }
    }

    fn move_table_selection_to_col_edge(&mut self, id: &str, end: bool) {
        let mut changed = false;
        if let Some(gpu) = &mut self.gpu {
            changed = gpu
                .widget_state
                .as_mut()
                .map(|ws| ws.move_table_selection_to_col_edge(id, end))
                .unwrap_or(false);
            if changed {
                gpu.rebuild_visuals();
            }
        }
        if changed {
            self.emit_current_table_selection(id);
            self.request_redraw();
        }
    }

    fn toggle_table_sort(&mut self, id: &str, col: usize) {
        if let Some(gpu) = &mut self.gpu {
            let changed = gpu
                .widget_state
                .as_mut()
                .map(|ws| ws.toggle_table_sort(id, col))
                .unwrap_or(false);
            if changed {
                gpu.refresh_table_sort(id);
                gpu.rebuild_visuals();
            }
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
        if matches!(&event.logical_key, Key::Named(NamedKey::Escape)) {
            if self.gpu.as_mut().is_some_and(WgpuState::close_popups) {
                self.request_redraw();
                return;
            }
            if let Some(closed) = self.gpu.as_mut().and_then(WgpuState::close_active_modal) {
                self.set_focus(None);
                self.record_runtime_command(
                    "ModalClose",
                    Some(closed),
                    Some("escape".to_string()),
                    Some(Dirty::Layout),
                    "applied",
                    true,
                );
                self.request_redraw();
                return;
            }
        }

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
                    if self.handle_text_input_key(&id, &event, false) {
                        return;
                    }
                }
                WidgetKind::TextArea => {
                    if self.handle_text_input_key(&id, &event, true) {
                        return;
                    }
                }
                WidgetKind::NumberInput => {
                    if self.handle_number_input_key(&id, &event) {
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
                WidgetKind::Menu => match &event.logical_key {
                    Key::Named(NamedKey::Enter)
                    | Key::Named(NamedKey::Space)
                    | Key::Named(NamedKey::ArrowDown) => {
                        self.activate_widget(&id, WidgetKind::Menu);
                        return;
                    }
                    Key::Named(NamedKey::Escape) => {
                        if self.gpu.as_mut().is_some_and(WgpuState::close_popups) {
                            self.request_redraw();
                            return;
                        }
                    }
                    _ => {}
                },
                WidgetKind::Button | WidgetKind::Checkbox | WidgetKind::Collapsible => {
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
                    let visible_counts = self
                        .gpu
                        .as_ref()
                        .and_then(|g| g.table_visible_counts(&id))
                        .unwrap_or((1, 1));
                    match &event.logical_key {
                        Key::Named(NamedKey::ArrowUp) => {
                            self.move_table_selection(
                                &id,
                                -1,
                                0,
                                visible_counts.0,
                                visible_counts.1,
                            );
                            return;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            self.move_table_selection(
                                &id,
                                1,
                                0,
                                visible_counts.0,
                                visible_counts.1,
                            );
                            return;
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.move_table_selection(
                                &id,
                                0,
                                -1,
                                visible_counts.0,
                                visible_counts.1,
                            );
                            return;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.move_table_selection(
                                &id,
                                0,
                                1,
                                visible_counts.0,
                                visible_counts.1,
                            );
                            return;
                        }
                        Key::Named(NamedKey::PageUp) => {
                            self.move_table_selection(
                                &id,
                                -(visible_counts.0.max(1) as isize),
                                0,
                                visible_counts.0,
                                visible_counts.1,
                            );
                            return;
                        }
                        Key::Named(NamedKey::PageDown) => {
                            self.move_table_selection(
                                &id,
                                visible_counts.0.max(1) as isize,
                                0,
                                visible_counts.0,
                                visible_counts.1,
                            );
                            return;
                        }
                        Key::Named(NamedKey::Home) => {
                            self.move_table_selection_to_col_edge(&id, false);
                            return;
                        }
                        Key::Named(NamedKey::End) => {
                            self.move_table_selection_to_col_edge(&id, true);
                            return;
                        }
                        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                            if self.emit_current_table_selection(&id) {
                                self.request_redraw();
                            }
                            return;
                        }
                        _ => {}
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

    fn adjust_number_input(&mut self, id: &str, direction: f32) {
        let changed = self
            .gpu
            .as_mut()
            .and_then(|g| g.widget_state.as_mut())
            .and_then(|ws| {
                let old = ws.float_val.get(id).copied();
                let value = ws.adjust_number(id, direction)?;
                let changed = old
                    .map(|old| (old - value).abs() > SLIDER_CHANGE_EPSILON)
                    .unwrap_or(true);
                changed.then_some(value)
            });
        if let Some(value) = changed {
            self.emit_change(id, ChangeValue::Float(value));
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.rebuild_visuals();
        }
        self.request_redraw();
    }

    fn process_number_text_change(&mut self, id: &str) {
        let changed = self
            .gpu
            .as_mut()
            .and_then(|g| g.widget_state.as_mut())
            .and_then(|ws| {
                let old = ws.float_val.get(id).copied();
                let value = ws.validate_number_text(id)??;
                let changed = old
                    .map(|old| (old - value).abs() > SLIDER_CHANGE_EPSILON)
                    .unwrap_or(true);
                changed.then_some(value)
            });
        if let Some(value) = changed {
            self.emit_change(id, ChangeValue::Float(value));
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.rebuild_visuals();
        }
        self.request_redraw();
    }

    fn handle_number_input_key(&mut self, id: &str, event: &winit::event::KeyEvent) -> bool {
        let mut changed_text = false;
        let mut handled = true;
        match &event.logical_key {
            Key::Named(NamedKey::ArrowUp) => {
                self.adjust_number_input(id, 1.0);
                return true;
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.adjust_number_input(id, -1.0);
                return true;
            }
            Key::Named(NamedKey::Enter) => {
                let committed = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| {
                        let value = ws.validate_number_text(id)??;
                        ws.set_number_value(id, value)
                    });
                if let Some(value) = committed {
                    self.emit_change(id, ChangeValue::Float(value));
                }
                if let Some(gpu) = &mut self.gpu {
                    gpu.rebuild_visuals();
                }
                self.request_redraw();
                return true;
            }
            Key::Named(NamedKey::Backspace) => {
                changed_text = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.backspace_text(id))
                    .is_some();
            }
            Key::Named(NamedKey::Delete) => {
                changed_text = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.delete_text(id))
                    .is_some();
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
                changed_text = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.insert_text(id, text))
                    .is_some();
                handled = true;
            }
        }

        if changed_text {
            self.process_number_text_change(id);
        } else if handled {
            if let Some(gpu) = &mut self.gpu {
                gpu.rebuild_visuals();
            }
            self.request_redraw();
        }
        handled
    }

    fn handle_text_input_key(
        &mut self,
        id: &str,
        event: &winit::event::KeyEvent,
        multiline: bool,
    ) -> bool {
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
            Key::Named(NamedKey::ArrowUp) if multiline => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.move_text_cursor_vertical(id, -1);
                    }
                }
            }
            Key::Named(NamedKey::ArrowDown) if multiline => {
                if let Some(gpu) = &mut self.gpu {
                    if let Some(ws) = &mut gpu.widget_state {
                        ws.move_text_cursor_vertical(id, 1);
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
            Key::Named(NamedKey::Enter) if multiline => {
                changed = self
                    .gpu
                    .as_mut()
                    .and_then(|g| g.widget_state.as_mut())
                    .and_then(|ws| ws.insert_text(id, "\n"));
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
            let insertable = event.text.as_deref().filter(|text| {
                if multiline {
                    is_insert_multiline_text(text)
                } else {
                    is_insert_text(text)
                }
            });
            if let Some(text) = insertable {
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
                if multiline {
                    gpu.ensure_text_area_cursor_visible(id);
                }
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

fn is_insert_multiline_text(text: &str) -> bool {
    !text.is_empty()
        && !text
            .chars()
            .any(|ch| ch == '\r' || ch == '\t' || (ch.is_control() && ch != '\n'))
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut request_redraw = false;
        let mut next_deadline = None;
        if let Some(gpu) = &mut self.gpu {
            if gpu.expire_toasts() {
                gpu.rebuild_visuals();
                request_redraw = true;
            }
            if gpu.tick_hover_transitions() {
                gpu.rebuild_visuals();
                request_redraw = true;
            }
            if gpu.tick_open_transitions() {
                gpu.rebuild_visuals();
                request_redraw = true;
            }
            if gpu.tick_selected_transitions() {
                gpu.rebuild_visuals();
                request_redraw = true;
            }
            next_deadline = gpu.next_toast_deadline();
            if gpu.has_style_transitions() {
                let transition_deadline = Instant::now() + Duration::from_millis(16);
                next_deadline = Some(
                    next_deadline
                        .map(|deadline| deadline.min(transition_deadline))
                        .unwrap_or(transition_deadline),
                );
            }
        }
        if request_redraw {
            self.request_redraw();
        }
        if let Some(deadline) = next_deadline {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
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
                            let was_orbiting = self.orbit_active;
                            let scatter_press = self.scatter_press_pos.take();
                            self.orbit_active = false;
                            let released_slider =
                                self.slider_drag.as_ref().map(|drag| drag.widget_id.clone());
                            if let Some(id) = released_slider {
                                self.flush_slider_change(&id);
                            }
                            self.slider_drag = None;

                            if was_orbiting {
                                let pos = self.last_mouse_pos.unwrap_or([0.0, 0.0]);
                                let moved2 = scatter_press
                                    .map(|start| {
                                        let dx = pos[0] - start[0];
                                        let dy = pos[1] - start[1];
                                        dx * dx + dy * dy
                                    })
                                    .unwrap_or(f32::INFINITY);
                                if moved2 <= 16.0 {
                                    if let Some((id, payload)) =
                                        self.gpu.as_ref().and_then(|g| g.scatter_pick_payload(pos))
                                    {
                                        self.emit_change(&id, ChangeValue::Text(payload));
                                    }
                                }
                            }

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
                                if over.as_deref() == Some(pid.as_str()) {
                                    if let Some(kind) =
                                        self.gpu.as_ref().and_then(|g| g.widget_kind(&pid))
                                    {
                                        self.activate_widget(&pid, kind);
                                    }
                                }
                                // Rebuild to clear pressed / update checkbox.
                                if let Some(gpu) = &mut self.gpu {
                                    gpu.rebuild_visuals();
                                }
                                self.request_redraw();
                            }
                        } else {
                            // ── press ─────────────────────────────────────────
                            let pos = self.last_mouse_pos.unwrap_or([0.0, 0.0]);
                            if self
                                .gpu
                                .as_ref()
                                .map(|g| g.modal_blocks_point(pos))
                                .unwrap_or(false)
                            {
                                self.set_focus(None);
                                return;
                            }
                            let modal_active = self
                                .gpu
                                .as_ref()
                                .map(WgpuState::has_active_modal)
                                .unwrap_or(false);
                            if !modal_active {
                                if let Some(item_id) =
                                    self.gpu.as_ref().and_then(|g| g.menu_item_at(pos))
                                {
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.close_popups();
                                    }
                                    self.emit_click(&item_id);
                                    self.request_redraw();
                                    return;
                                }

                                let popup_open = self
                                    .gpu
                                    .as_ref()
                                    .map(WgpuState::has_open_menu_popup)
                                    .unwrap_or(false);
                                if popup_open
                                    && !self
                                        .gpu
                                        .as_ref()
                                        .map(|g| g.menu_popup_contains(pos))
                                        .unwrap_or(false)
                                {
                                    let over_menu = self
                                        .gpu
                                        .as_ref()
                                        .and_then(|g| g.hit_test_ui(pos))
                                        .is_some_and(|(_, kind)| kind == WidgetKind::Menu);
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.close_popups();
                                    }
                                    if !over_menu {
                                        self.set_focus(None);
                                        self.request_redraw();
                                        return;
                                    }
                                }
                            }
                            if let Some((id, idx)) = (!modal_active)
                                .then(|| self.gpu.as_ref().and_then(|g| g.dropdown_option_at(pos)))
                                .flatten()
                            {
                                self.set_focus(Some(id.clone()));
                                self.select_dropdown_option(&id, idx);
                                return;
                            }

                            if let Some((id, hit)) = (!modal_active)
                                .then(|| self.gpu.as_ref().and_then(|g| g.table_hit(pos)))
                                .flatten()
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

                            if let Some((id, direction)) = (!modal_active)
                                .then(|| {
                                    self.gpu.as_ref().and_then(|g| g.number_input_step_at(pos))
                                })
                                .flatten()
                            {
                                self.set_focus(Some(id.clone()));
                                self.adjust_number_input(&id, direction);
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
                                    gpu.rebuild_visuals();
                                }
                                if kind == WidgetKind::Slider {
                                    self.update_slider_drag(pos[0], true);
                                }
                                self.request_redraw();
                                return;
                            }

                            if modal_active {
                                self.set_focus(None);
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
                                self.scatter_press_pos = Some(pos);
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
                            if self
                                .gpu
                                .as_ref()
                                .map(WgpuState::has_active_modal)
                                .unwrap_or(false)
                            {
                                return;
                            }
                            if button == MouseButton::Right {
                                if let Some(menu_id) =
                                    self.gpu.as_ref().and_then(|g| g.context_menu_for_pos(pos))
                                {
                                    self.set_focus(None);
                                    if let Some(gpu) = &mut self.gpu {
                                        gpu.open_context_menu_at(&menu_id, pos);
                                    }
                                    self.request_redraw();
                                    return;
                                }
                                if self.gpu.as_mut().is_some_and(WgpuState::close_popups) {
                                    self.request_redraw();
                                    return;
                                }
                            }
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

            WindowEvent::CursorLeft { .. } => {
                self.last_mouse_pos = None;
                if let Some(gpu) = &mut self.gpu {
                    let cleared = gpu.update_hover_state(None, None);
                    if cleared {
                        gpu.apply_layout();
                        self.request_redraw();
                    }
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
                    let new_dropdown_hover = self
                        .gpu
                        .as_ref()
                        .and_then(|g| g.dropdown_option_at(new_pos));
                    let new_hover = if new_dropdown_hover.is_some() {
                        None
                    } else {
                        self.gpu
                            .as_ref()
                            .and_then(|g| g.hit_test_hover(new_pos))
                            .map(|(id, _)| id)
                    };
                    let old_hover = self
                        .gpu
                        .as_ref()
                        .and_then(|g| g.widget_state.as_ref())
                        .and_then(|ws| ws.hovered.clone());
                    let old_dropdown_hover = self
                        .gpu
                        .as_ref()
                        .and_then(|g| g.widget_state.as_ref())
                        .and_then(|ws| ws.dropdown_hover.clone());
                    if new_hover != old_hover || new_dropdown_hover != old_dropdown_hover {
                        if let Some(gpu) = &mut self.gpu {
                            gpu.update_hover_state(new_hover, new_dropdown_hover);
                            // Rich tooltip content participates in overlay layout, so hover
                            // changes can affect rects as well as paint/text state.
                            gpu.apply_layout();
                        }
                        self.request_redraw();
                    }
                }

                self.last_mouse_pos = Some(new_pos);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if self
                    .gpu
                    .as_ref()
                    .map(WgpuState::has_active_modal)
                    .unwrap_or(false)
                {
                    return;
                }
                let (scroll_x, scroll_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 * 0.01, pos.y as f32 * 0.01),
                };
                if let Some(pos) = self.last_mouse_pos {
                    if let Some(id) = self.gpu.as_ref().and_then(|gpu| gpu.text_area_at(pos)) {
                        if self
                            .gpu
                            .as_mut()
                            .is_some_and(|gpu| gpu.scroll_text_area(&id, scroll_y))
                        {
                            self.request_redraw();
                        }
                        return;
                    }
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
                    if let Some(id) = self
                        .gpu
                        .as_ref()
                        .and_then(|gpu| gpu.scroll_container_at(pos))
                    {
                        if self
                            .gpu
                            .as_mut()
                            .is_some_and(|gpu| gpu.scroll_container(&id, scroll_y))
                        {
                            self.request_redraw();
                        }
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
                if let Some((id, kind)) = focused {
                    let insertable = match kind {
                        WidgetKind::TextArea => is_insert_multiline_text(&text),
                        _ => is_insert_text(&text),
                    };
                    if insertable {
                        let changed = self
                            .gpu
                            .as_mut()
                            .and_then(|g| g.widget_state.as_mut())
                            .and_then(|ws| ws.insert_text(&id, &text));
                        match kind {
                            WidgetKind::TextInput => {
                                if let Some(value) = changed {
                                    self.emit_change(&id, ChangeValue::Text(value));
                                }
                                if let Some(gpu) = &mut self.gpu {
                                    gpu.rebuild_visuals();
                                }
                                self.request_redraw();
                            }
                            WidgetKind::TextArea => {
                                if let Some(value) = changed {
                                    self.emit_change(&id, ChangeValue::Text(value));
                                }
                                if let Some(gpu) = &mut self.gpu {
                                    gpu.ensure_text_area_cursor_visible(&id);
                                    gpu.rebuild_visuals();
                                }
                                self.request_redraw();
                            }
                            WidgetKind::NumberInput => {
                                if changed.is_some() {
                                    self.process_number_text_change(&id);
                                }
                            }
                            _ => {}
                        }
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
