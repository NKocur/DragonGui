use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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
use winit::window::{Theme as WinitTheme, Window, WindowId};

use crate::commands::{
    Command, CommandBridge, CommandValue, Dirty, RuntimeEvent, ScatterTelemetry, TableColumnPacket,
};
use crate::css_style::{
    apply_stylesheets_to_tree_for_media, matched_part_rule_labels_for_tree_with_media,
    matched_rule_labels_for_tree_with_media, DgKeyframes, DgMediaColorGamut, DgMediaColorScheme,
    DgMediaEnvironment, DgMediaHover, DgMediaPointer, StylesheetOrigin, StylesheetStore,
};
use crate::document::ScatterPayloadFormat;
use crate::document::{self, NodeProps, WidgetKind, WidgetNode};
use crate::error::DragonError;
use crate::events::{
    has_active_modal, hit_test, hit_test_hover, modal_blocks_point, ChangeValue, SliderDrag,
    WidgetState,
};
use crate::image_widget::ImageRenderer;
use crate::layout::{
    compute_layout, is_scroll_container_node, scroll_container_max_x, scroll_container_max_y, Rect,
};
use crate::overlays::{find_node, menu_popup_width};
use crate::primitives::{
    interpolate_visual_style, panel_scrollbar_geometry, PanelScrollbarAxis,
    PanelScrollbarAxisGeometry, PrimitivesRenderer,
};
use crate::resources::ResourceRegistry;
use crate::scatter::{self, PointInstance, ScatterWidget};
use crate::style::{
    collapsible_header_height_for_style, number_stepper_width, number_stepper_width_for_style,
    AlignItemsStyle, AnimationDirection, AnimationFillMode, AnimationIterationCount,
    AnimationPlayState, AnimationStyle, BackgroundPaint, BoxShadow, ColorRef, DisplayStyle,
    FlexDirectionStyle, FontFamily, FontStyle, FontVariantNumeric, GeneratedContent,
    GridAutoFlowStyle, GridLineStyle, GridPlacementStyle, GridTrackSize, LayoutLength, LayoutStyle,
    LineHeight, NodeStyle, OverflowStyle, PartLayoutStyle, PartStyle, PositionStyle, StepPosition,
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
const MAX_COMMANDS_PER_DRAIN_BATCH: usize = 32;
const COMMAND_DRAIN_BUDGET: Duration = Duration::from_millis(6);

fn coalesce_runtime_command_batch(commands: &mut Vec<Command>) {
    if commands.len() < 2 {
        return;
    }
    let mut seen_scatter_updates = HashSet::new();
    let mut filtered = Vec::with_capacity(commands.len());
    while let Some(command) = commands.pop() {
        let keep = match &command {
            Command::DebugSnapshot { .. } => {
                seen_scatter_updates.clear();
                true
            }
            Command::SetScatterPointsPacked {
                id, coalesce: true, ..
            } => seen_scatter_updates.insert(id.clone()),
            _ => true,
        };
        if keep {
            filtered.push(command);
        }
    }
    filtered.reverse();
    *commands = filtered;
}

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

fn decode_mesh_positions(b64: &str) -> Result<Vec<[f32; 3]>, DragonError> {
    let bytes = BASE64
        .decode(b64)
        .map_err(|e| DragonError::ParseError(format!("mesh positions base64: {e}")))?;
    if bytes.len() % 12 != 0 {
        return Err(DragonError::ParseError(format!(
            "mesh positions length {} is not a multiple of 12 (xyz float32)",
            bytes.len()
        )));
    }
    Ok(bytes
        .chunks_exact(12)
        .map(|c| {
            [
                f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                f32::from_le_bytes([c[4], c[5], c[6], c[7]]),
                f32::from_le_bytes([c[8], c[9], c[10], c[11]]),
            ]
        })
        .collect())
}

fn decode_mesh_indices(b64: &str) -> Result<Vec<[u32; 3]>, DragonError> {
    let bytes = BASE64
        .decode(b64)
        .map_err(|e| DragonError::ParseError(format!("mesh indices base64: {e}")))?;
    if bytes.len() % 12 != 0 {
        return Err(DragonError::ParseError(format!(
            "mesh indices length {} is not a multiple of 12 (3 × uint32 per triangle)",
            bytes.len()
        )));
    }
    Ok(bytes
        .chunks_exact(12)
        .map(|c| {
            [
                u32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                u32::from_le_bytes([c[4], c[5], c[6], c[7]]),
                u32::from_le_bytes([c[8], c[9], c[10], c[11]]),
            ]
        })
        .collect())
}

fn decode_scatter_points(b64: &str, colormap: &str) -> Result<Vec<PointInstance>, DragonError> {
    let bytes = BASE64
        .decode(b64)
        .map_err(|e| DragonError::ParseError(format!("scatter data base64: {e}")))?;
    let mut pts = Vec::new();
    decode_scatter_points_bytes_into_colormap(&bytes, &mut pts, colormap)?;
    Ok(pts)
}

/// Format a scalar like Python's :.4g — 4 significant figures, scientific when |exp| ≥ 4.
fn format_4g(v: f32) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let abs = v.abs();
    if abs >= 1e-4_f32 && abs < 1e4_f32 {
        let exp = abs.log10().floor() as i32;
        let prec = (3 - exp).max(0) as usize;
        // Remove redundant trailing zeros after decimal point.
        let s = format!("{:.prec$}", v, prec = prec);
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    } else {
        format!("{:.3e}", v)
    }
}

fn decode_actor_payload(
    b64: &str,
    colormap: &str,
    format: ScatterPayloadFormat,
) -> Result<Vec<PointInstance>, DragonError> {
    let bytes = BASE64
        .decode(b64)
        .map_err(|e| DragonError::ParseError(format!("scatter actor base64: {e}")))?;
    decode_actor_payload_bytes(&bytes, colormap, format)
}

fn decode_actor_payload_bytes(
    bytes: &[u8],
    colormap: &str,
    format: ScatterPayloadFormat,
) -> Result<Vec<PointInstance>, DragonError> {
    let mut pts = Vec::new();
    match format {
        ScatterPayloadFormat::XyzF32V0 => {
            decode_scatter_points_bytes_into_colormap(bytes, &mut pts, colormap)?;
        }
        ScatterPayloadFormat::PointInstanceV1 => {
            decode_scatter_points_v1(bytes, &mut pts)?;
        }
    }
    Ok(pts)
}

fn decode_scatter_points_bytes_into_colormap(
    bytes: &[u8],
    pts: &mut Vec<PointInstance>,
    colormap: &str,
) -> Result<Option<(glam::Vec3, glam::Vec3)>, DragonError> {
    if bytes.len() % 12 != 0 {
        return Err(DragonError::ParseError(format!(
            "scatter data length {} is not a multiple of 12 (xyz float32)",
            bytes.len()
        )));
    }

    let n = bytes.len() / 12;
    pts.clear();
    pts.reserve(n);

    // Decode once while computing z range and data bounds. Color is filled after
    // the final z range is known so xyz_f32_v0 keeps exact auto-colormap behavior.
    let mut z_min = f32::INFINITY;
    let mut z_max = f32::NEG_INFINITY;
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for i in 0..n {
        let off = i * 12;
        let x = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let y = f32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
        let z = f32::from_le_bytes(bytes[off + 8..off + 12].try_into().unwrap());
        if z.is_finite() {
            z_min = z_min.min(z);
            z_max = z_max.max(z);
        }
        if x.is_finite() && y.is_finite() && z.is_finite() {
            let p = glam::Vec3::new(x, y, z);
            min = min.min(p);
            max = max.max(p);
        }
        pts.push(PointInstance {
            position: [x, y, z],
            size: 3.0,
            color: [0.0, 0.0, 0.0],
            alpha: 0.85,
        });
    }
    let z_range = if z_max > z_min { z_max - z_min } else { 1.0 };
    let bounds = if min.x > max.x {
        None
    } else {
        Some((min, max))
    };

    // Assign colors by normalized z position within the range.
    let cmap = scatter::colormap::resolve(colormap);
    for pt in pts.iter_mut() {
        let z = pt.position[2];
        let t = if z.is_finite() {
            ((z - z_min) / z_range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let [r, g, b] = scatter::colormap::sample(cmap, t);
        pt.color = [r, g, b];
    }

    Ok(bounds)
}

fn decode_scatter_points_v1(
    bytes: &[u8],
    pts: &mut Vec<PointInstance>,
) -> Result<Option<(glam::Vec3, glam::Vec3)>, DragonError> {
    const STRIDE: usize = std::mem::size_of::<PointInstance>();
    if bytes.len() % STRIDE != 0 {
        return Err(DragonError::ParseError(format!(
            "scatter point_instance_v1 payload length {} is not a multiple of {} (PointInstance size)",
            bytes.len(),
            STRIDE
        )));
    }
    let n = bytes.len() / STRIDE;
    pts.clear();
    pts.reserve(n);
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for i in 0..n {
        let off = i * STRIDE;
        let x = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        let y = f32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
        let z = f32::from_le_bytes(bytes[off + 8..off + 12].try_into().unwrap());
        let size = f32::from_le_bytes(bytes[off + 12..off + 16].try_into().unwrap());
        let r = f32::from_le_bytes(bytes[off + 16..off + 20].try_into().unwrap());
        let g = f32::from_le_bytes(bytes[off + 20..off + 24].try_into().unwrap());
        let b = f32::from_le_bytes(bytes[off + 24..off + 28].try_into().unwrap());
        let alpha = f32::from_le_bytes(bytes[off + 28..off + 32].try_into().unwrap());
        if x.is_finite() && y.is_finite() && z.is_finite() {
            let p = glam::Vec3::new(x, y, z);
            min = min.min(p);
            max = max.max(p);
        }
        pts.push(PointInstance {
            position: [x, y, z],
            size: size.max(0.0),
            color: [r, g, b],
            alpha,
        });
    }
    if min.x > max.x {
        Ok(None)
    } else {
        Ok(Some((min, max)))
    }
}

fn compute_scatter_bounds(pts: &[PointInstance]) -> Option<(glam::Vec3, glam::Vec3)> {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    for pt in pts {
        let [x, y, z] = pt.position;
        if x.is_finite() && y.is_finite() && z.is_finite() {
            min = min.min(glam::Vec3::new(x, y, z));
            max = max.max(glam::Vec3::new(x, y, z));
        }
    }
    if min.x > max.x {
        None
    } else {
        Some((min, max))
    }
}

fn scatter_chrome_from_props(props: &document::NodeProps) -> scatter::ScatterChromeState {
    scatter::ScatterChromeState {
        grid_visible: props.scatter_grid_visible,
        major_planes: props.scatter_major_planes,
        minor_planes: props.scatter_minor_planes,
        grid_sticky: props.scatter_grid_sticky,
        grid_all_edges: props.scatter_grid_all_edges,
        tick_override: props.scatter_tick_override,
        axis_labels: props.scatter_axis_labels.clone(),
        axis_visible: props.scatter_axis_visible,
        background_color: props.scatter_background,
        legend: scatter::LegendState {
            visible: props.scatter_legend_visible,
            position: scatter::LegendPosition::from_str(&props.scatter_legend_position),
            entries: props
                .scatter_legend_entries
                .iter()
                .map(|(label, r, g, b)| scatter::LegendEntry {
                    label: label.clone(),
                    color: [*r, *g, *b],
                })
                .collect(),
            title: props.scatter_legend_title.clone(),
        },
        scalar_bar: scatter::ScalarBarState {
            visible: props.scatter_scalar_bar_visible,
            vmin: props.scatter_scalar_bar_vmin,
            vmax: props.scatter_scalar_bar_vmax,
            log_scale: props.scatter_scalar_bar_log_scale,
            colormap: props.scatter_scalar_bar_colormap.clone(),
            title: props.scatter_scalar_bar_title.clone(),
        },
        orientation_axes_visible: props.scatter_orientation_axes_visible,
    }
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

fn collect_visible_scatter_ids(
    node: &WidgetNode,
    layout: &crate::layout::LayoutResult,
    out: &mut Vec<String>,
) {
    if node.kind == WidgetKind::Scatter3D && layout.visible_rect(&node.id).is_some() {
        out.push(node.id.clone());
    }
    // Visit children in z-index paint order (matching primitives/text stacking_children).
    let mut children: Vec<_> = node.children.iter().enumerate().collect();
    children.sort_by_key(|(index, child)| (child.style.layout.z_index.unwrap_or(0), *index));
    for (_, child) in children {
        collect_visible_scatter_ids(child, layout, out);
    }
}

fn collect_all_scatter_ids(node: &WidgetNode, out: &mut Vec<String>) {
    if node.kind == WidgetKind::Scatter3D {
        out.push(node.id.clone());
    }
    for child in &node.children {
        collect_all_scatter_ids(child, out);
    }
}

fn scatter_clip_radii(node: &WidgetNode, fallback_radius_lp: f32, scale_factor: f32) -> [f32; 4] {
    let radius_lp = node
        .style
        .visual
        .border_radius
        .unwrap_or(fallback_radius_lp)
        .max(0.0);
    node.style
        .visual
        .corner_radii
        .resolve(radius_lp)
        .map(|radius| radius.max(0.0) * scale_factor)
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

fn active_modal_ref(node: &WidgetNode) -> Option<&WidgetNode> {
    for child in node.children.iter().rev() {
        if let Some(modal) = active_modal_ref(child) {
            return Some(modal);
        }
    }
    (node.kind == WidgetKind::Modal && node.props.open.unwrap_or(false)).then_some(node)
}

fn has_rich_tooltip_for_target(node: &WidgetNode, target: &str) -> bool {
    if node.kind == WidgetKind::Tooltip && node.props.target.as_deref() == Some(target) {
        return true;
    }
    node.children
        .iter()
        .any(|child| has_rich_tooltip_for_target(child, target))
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
    let max_scroll_x = layout
        .scroll_max_x
        .get(&node.id)
        .copied()
        .unwrap_or_else(|| scroll_container_max_x(node, layout));
    let max_scroll_y = layout
        .scroll_max_y
        .get(&node.id)
        .copied()
        .unwrap_or_else(|| scroll_container_max_y(node, layout));
    (max_scroll_x > 0.0 || max_scroll_y > 0.0).then(|| node.id.clone())
}

#[derive(Clone, Debug)]
struct ScrollContainerKeyboardTarget {
    id: String,
    rect: Rect,
    current_x: f32,
    current_y: f32,
    max_x: f32,
    max_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScrollKeyboardCommand {
    PageBackward,
    PageForward,
    Start,
    End,
}

fn scroll_container_keyboard_target_by_id(
    node: &WidgetNode,
    layout: &crate::layout::LayoutResult,
    state: &WidgetState,
) -> Option<ScrollContainerKeyboardTarget> {
    if !is_scroll_container_node(node) || state.is_disabled(&node.id) {
        return None;
    }
    let max_x = layout
        .scroll_max_x
        .get(&node.id)
        .copied()
        .unwrap_or_else(|| scroll_container_max_x(node, layout));
    let max_y = layout
        .scroll_max_y
        .get(&node.id)
        .copied()
        .unwrap_or_else(|| scroll_container_max_y(node, layout));
    if max_x <= 0.0 && max_y <= 0.0 {
        return None;
    }
    let rect = layout.visible_rect(&node.id)?;
    Some(ScrollContainerKeyboardTarget {
        id: node.id.clone(),
        rect,
        current_x: state.container_scroll_x(&node.id, max_x),
        current_y: state.container_scroll_y(&node.id, max_y),
        max_x,
        max_y,
    })
}

fn focused_scroll_container_keyboard_target(
    node: &WidgetNode,
    layout: &crate::layout::LayoutResult,
    state: &WidgetState,
    focused_id: &str,
    ancestor: Option<ScrollContainerKeyboardTarget>,
) -> Option<ScrollContainerKeyboardTarget> {
    let ancestor = scroll_container_keyboard_target_by_id(node, layout, state).or(ancestor);
    if node.id == focused_id {
        return ancestor;
    }
    for child in &node.children {
        if let Some(target) = focused_scroll_container_keyboard_target(
            child,
            layout,
            state,
            focused_id,
            ancestor.clone(),
        ) {
            return Some(target);
        }
    }
    None
}

fn scroll_keyboard_command(key: &Key) -> Option<ScrollKeyboardCommand> {
    match key {
        Key::Named(NamedKey::PageUp) => Some(ScrollKeyboardCommand::PageBackward),
        Key::Named(NamedKey::PageDown) => Some(ScrollKeyboardCommand::PageForward),
        Key::Named(NamedKey::Home) => Some(ScrollKeyboardCommand::Start),
        Key::Named(NamedKey::End) => Some(ScrollKeyboardCommand::End),
        _ => None,
    }
}

fn scroll_keyboard_destination(
    target: &ScrollContainerKeyboardTarget,
    command: ScrollKeyboardCommand,
    shift: bool,
) -> Option<(PanelScrollbarAxis, f32)> {
    let axis = if shift && target.max_x > 0.0 {
        PanelScrollbarAxis::Horizontal
    } else if target.max_y > 0.0 {
        PanelScrollbarAxis::Vertical
    } else if target.max_x > 0.0 {
        PanelScrollbarAxis::Horizontal
    } else {
        return None;
    };
    let (current, max_scroll, page) = match axis {
        PanelScrollbarAxis::Horizontal => (target.current_x, target.max_x, target.rect.w),
        PanelScrollbarAxis::Vertical => (target.current_y, target.max_y, target.rect.h),
    };
    let page = (page * 0.85).max(1.0);
    let next = match command {
        ScrollKeyboardCommand::PageBackward => current - page,
        ScrollKeyboardCommand::PageForward => current + page,
        ScrollKeyboardCommand::Start => 0.0,
        ScrollKeyboardCommand::End => max_scroll,
    };
    Some((axis, next.clamp(0.0, max_scroll)))
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
        WidgetKind::ScrollArea => "scroll_area",
        WidgetKind::GridLayout => "grid_layout",
        WidgetKind::FlowLayout => "flow_layout",
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

fn align_items_name(align_items: AlignItemsStyle) -> &'static str {
    match align_items {
        AlignItemsStyle::Start => "start",
        AlignItemsStyle::Center => "center",
        AlignItemsStyle::End => "end",
        AlignItemsStyle::Stretch => "stretch",
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
        PositionStyle::Fixed => "fixed",
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

fn grid_track_json(value: &GridTrackSize) -> Value {
    match value {
        GridTrackSize::LogicalPx(value) => json!(*value),
        GridTrackSize::Percent(value) => json!({ "percent": *value }),
        GridTrackSize::Fraction(value) => json!({ "fr": *value }),
        GridTrackSize::Auto => json!("auto"),
        GridTrackSize::FitContent(value) => {
            json!({ "fit_content": grid_track_fit_content_json(*value) })
        }
        GridTrackSize::MinMax { min, max } => {
            json!({ "minmax": { "min": grid_track_min_json(*min), "max": grid_track_max_json(*max) } })
        }
        GridTrackSize::Repeat { kind, tracks } => {
            json!({
                "repeat": {
                    "kind": grid_track_repeat_kind_json(*kind),
                    "tracks": tracks.iter().map(grid_track_json).collect::<Vec<_>>(),
                }
            })
        }
    }
}

fn grid_track_repeat_kind_json(value: crate::style::GridTrackRepeatKind) -> &'static str {
    match value {
        crate::style::GridTrackRepeatKind::AutoFit => "auto-fit",
        crate::style::GridTrackRepeatKind::AutoFill => "auto-fill",
    }
}

fn grid_track_fit_content_json(value: crate::style::GridTrackFitContentSize) -> Value {
    match value {
        crate::style::GridTrackFitContentSize::LogicalPx(value) => json!(value),
        crate::style::GridTrackFitContentSize::Percent(value) => json!({ "percent": value }),
    }
}

fn grid_track_min_json(value: crate::style::GridTrackMinSize) -> Value {
    match value {
        crate::style::GridTrackMinSize::LogicalPx(value) => json!(value),
        crate::style::GridTrackMinSize::Percent(value) => json!({ "percent": value }),
        crate::style::GridTrackMinSize::Auto => json!("auto"),
    }
}

fn grid_track_max_json(value: crate::style::GridTrackMaxSize) -> Value {
    match value {
        crate::style::GridTrackMaxSize::LogicalPx(value) => json!(value),
        crate::style::GridTrackMaxSize::Percent(value) => json!({ "percent": value }),
        crate::style::GridTrackMaxSize::Fraction(value) => json!({ "fr": value }),
        crate::style::GridTrackMaxSize::Auto => json!("auto"),
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

fn grid_auto_flow_name(value: GridAutoFlowStyle) -> &'static str {
    match value {
        GridAutoFlowStyle::Row => "row",
        GridAutoFlowStyle::Column => "column",
        GridAutoFlowStyle::RowDense => "row dense",
        GridAutoFlowStyle::ColumnDense => "column dense",
    }
}

fn grid_template_areas_json(value: &crate::style::GridTemplateAreas) -> Value {
    json!({
        "columns": value.columns,
        "rows": value.rows,
        "areas": value
            .areas
            .iter()
            .map(|area| {
                json!({
                    "name": area.name,
                    "row_start": area.row_start,
                    "row_end": area.row_end,
                    "column_start": area.column_start,
                    "column_end": area.column_end,
                })
            })
            .collect::<Vec<_>>(),
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
    if let Some(value) = style.align_items {
        map.insert("align_items".to_string(), json!(align_items_name(value)));
    }
    if let Some(value) = style.align_self {
        map.insert("align_self".to_string(), json!(align_items_name(value)));
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
    insert_layout_length(&mut map, "padding", style.padding_value, style.padding);
    insert_layout_length(
        &mut map,
        "padding_left",
        style.padding_left_value,
        style.padding_left,
    );
    insert_layout_length(
        &mut map,
        "padding_right",
        style.padding_right_value,
        style.padding_right,
    );
    insert_layout_length(
        &mut map,
        "padding_top",
        style.padding_top_value,
        style.padding_top,
    );
    insert_layout_length(
        &mut map,
        "padding_bottom",
        style.padding_bottom_value,
        style.padding_bottom,
    );
    insert_layout_length(&mut map, "margin", style.margin_value, style.margin);
    insert_layout_length(
        &mut map,
        "margin_left",
        style.margin_left_value,
        style.margin_left,
    );
    insert_layout_length(
        &mut map,
        "margin_right",
        style.margin_right_value,
        style.margin_right,
    );
    insert_layout_length(
        &mut map,
        "margin_top",
        style.margin_top_value,
        style.margin_top,
    );
    insert_layout_length(
        &mut map,
        "margin_bottom",
        style.margin_bottom_value,
        style.margin_bottom,
    );
    insert_layout_length(&mut map, "gap", style.gap_value, style.gap);
    insert_layout_length(&mut map, "row_gap", style.row_gap_value, style.row_gap);
    insert_layout_length(
        &mut map,
        "column_gap",
        style.column_gap_value,
        style.column_gap,
    );
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
            Value::Array(value.iter().map(grid_track_json).collect()),
        );
    }
    if let Some(value) = &style.grid_template_rows {
        map.insert(
            "grid_template_rows".to_string(),
            Value::Array(value.iter().map(grid_track_json).collect()),
        );
    }
    if let Some(value) = &style.grid_template_areas {
        map.insert(
            "grid_template_areas".to_string(),
            grid_template_areas_json(value),
        );
    }
    if let Some(value) = style.grid_auto_flow {
        map.insert(
            "grid_auto_flow".to_string(),
            json!(grid_auto_flow_name(value)),
        );
    }
    if let Some(value) = &style.grid_area {
        map.insert("grid_area".to_string(), json!(value));
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
    if let Some(interpolation) = style.gradient_interpolation {
        let value = match interpolation {
            crate::style::GradientInterpolation::Srgb => "srgb",
            crate::style::GradientInterpolation::LinearSrgb => "linear-srgb",
            crate::style::GradientInterpolation::Oklab => "oklab",
        };
        map.insert("gradient_interpolation".to_string(), json!(value));
    }
    if let Some(filter) = style.backdrop_filter {
        map.insert(
            "backdrop_filter".to_string(),
            json!({
                "blur": filter.blur,
                "brightness": filter.brightness,
                "saturate": filter.saturate,
            }),
        );
    }
    insert_color_ref(&mut map, "foreground", &style.foreground);
    insert_color_ref(&mut map, "border_color", &style.border_color);
    insert_number(&mut map, "border_width", style.border_width);
    insert_color_ref(&mut map, "outline_color", &style.outline_color);
    insert_number(&mut map, "outline_width", style.outline_width);
    insert_number(&mut map, "outline_offset", style.outline_offset);
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
        BackgroundPaint::BlobGradient(gradient) => Some(json!({
            "type": "blob_gradient",
            "blobs": gradient.blobs.iter().map(|blob| {
                json!({
                    "center": blob.center,
                    "radius": blob.radius,
                    "color": color_ref_json(&blob.color),
                })
            }).collect::<Vec<_>>(),
        })),
        BackgroundPaint::MeshGradient(gradient) => Some(json!({
            "type": "mesh_gradient",
            "top_left": color_ref_json(&gradient.top_left),
            "top_right": color_ref_json(&gradient.top_right),
            "bottom_left": color_ref_json(&gradient.bottom_left),
            "bottom_right": color_ref_json(&gradient.bottom_right),
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

fn animation_style_snapshot(style: &AnimationStyle) -> Value {
    let mut map = Map::new();
    if let Some(name) = &style.name {
        map.insert("name".to_string(), json!(name));
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
    if let Some(count) = style.iteration_count {
        map.insert(
            "iteration_count".to_string(),
            animation_iteration_json(count),
        );
    }
    if let Some(direction) = style.direction {
        map.insert(
            "direction".to_string(),
            json!(animation_direction_name(direction)),
        );
    }
    if let Some(fill_mode) = style.fill_mode {
        map.insert(
            "fill_mode".to_string(),
            json!(animation_fill_mode_name(fill_mode)),
        );
    }
    if let Some(play_state) = style.play_state {
        map.insert(
            "play_state".to_string(),
            json!(animation_play_state_name(play_state)),
        );
    }
    Value::Object(map)
}

fn animation_iteration_json(value: AnimationIterationCount) -> Value {
    match value {
        AnimationIterationCount::One => json!(1),
        AnimationIterationCount::Infinite => json!("infinite"),
        AnimationIterationCount::Count(count) => json!(count),
    }
}

fn animation_direction_name(value: AnimationDirection) -> &'static str {
    match value {
        AnimationDirection::Normal => "normal",
        AnimationDirection::Reverse => "reverse",
        AnimationDirection::Alternate => "alternate",
        AnimationDirection::AlternateReverse => "alternate-reverse",
    }
}

fn animation_fill_mode_name(value: AnimationFillMode) -> &'static str {
    match value {
        AnimationFillMode::None => "none",
        AnimationFillMode::Forwards => "forwards",
        AnimationFillMode::Backwards => "backwards",
        AnimationFillMode::Both => "both",
    }
}

fn animation_play_state_name(value: AnimationPlayState) -> &'static str {
    match value {
        AnimationPlayState::Running => "running",
        AnimationPlayState::Paused => "paused",
    }
}

fn transition_property_name(property: crate::style::TransitionProperty) -> &'static str {
    match property {
        crate::style::TransitionProperty::All => "all",
        crate::style::TransitionProperty::Background => "background",
        crate::style::TransitionProperty::Foreground => "foreground",
        crate::style::TransitionProperty::BorderColor => "border-color",
        crate::style::TransitionProperty::BorderWidth => "border-width",
        crate::style::TransitionProperty::BorderRadius => "border-radius",
        crate::style::TransitionProperty::Outline => "outline",
        crate::style::TransitionProperty::OutlineColor => "outline-color",
        crate::style::TransitionProperty::OutlineWidth => "outline-width",
        crate::style::TransitionProperty::OutlineOffset => "outline-offset",
        crate::style::TransitionProperty::Opacity => "opacity",
        crate::style::TransitionProperty::Color => "color",
        crate::style::TransitionProperty::Accent => "accent",
        crate::style::TransitionProperty::TrackColor => "track-color",
        crate::style::TransitionProperty::ThumbColor => "thumb-color",
        crate::style::TransitionProperty::BoxShadow => "box-shadow",
        crate::style::TransitionProperty::Transform => "transform",
    }
}

fn transition_timing_name(timing: TransitionTimingFunction) -> String {
    timing.css_text()
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
    insert_number(&mut map, "text_area_rows", style.text_area_rows);
    insert_number(&mut map, "scatter_point_size", style.scatter_point_size);
    if let Some(ref sty) = style.scatter_point_style {
        map.insert(
            "scatter_point_style".to_string(),
            Value::String(sty.clone()),
        );
    }
    insert_number(&mut map, "table_row_height", style.table_row_height);
    insert_number(&mut map, "table_header_height", style.table_header_height);
    insert_number(&mut map, "table_column_width", style.table_column_width);
    insert_number(&mut map, "table_index_width", style.table_index_width);
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
    if let Some(content) = &style.content {
        map.insert(
            "content".to_string(),
            json!(generated_content_snapshot(content)),
        );
    }
    Value::Object(map)
}

fn generated_content_snapshot(content: &GeneratedContent) -> String {
    match content {
        GeneratedContent::Text(value) => value.clone(),
        GeneratedContent::Attr(name) => format!("attr({name})"),
    }
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
        "animation",
        animation_style_snapshot(&style.animation),
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

fn computed_styles_snapshot(
    root: Option<&WidgetNode>,
    store: &StylesheetStore,
    media: Option<DgMediaEnvironment>,
) -> Value {
    let Some(root) = root else {
        return json!({});
    };
    let matched_rules = matched_rule_labels_for_tree_with_media(root, store, media);
    let matched_part_rules = matched_part_rule_labels_for_tree_with_media(root, store, media);
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
        "grid_columns": props.grid_columns,
        "grid_min_column_width": props.grid_min_column_width,
        "flow_align": props.flow_align.as_deref(),
        "flow_cross_align": props.flow_cross_align.as_deref(),
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
    let mut clips = Map::new();
    for (id, rect) in &layout.clips {
        clips.insert(id.clone(), rect_json(*rect));
    }
    let mut diagnostics = Map::new();
    for (id, rect) in &layout.rects {
        let clip = layout.clips.get(id).copied().unwrap_or(*rect);
        let overflow_left = (clip.x - rect.x).max(0.0);
        let overflow_top = (clip.y - rect.y).max(0.0);
        let overflow_right = ((rect.x + rect.w) - (clip.x + clip.w)).max(0.0);
        let overflow_bottom = ((rect.y + rect.h) - (clip.y + clip.h)).max(0.0);
        diagnostics.insert(
            id.clone(),
            json!({
                "resolved": {
                    "x": rect.x,
                    "y": rect.y,
                    "width": rect.w,
                    "height": rect.h,
                },
                "available": {
                    "x": clip.x,
                    "y": clip.y,
                    "width": clip.w,
                    "height": clip.h,
                },
                "overflow": {
                    "left": overflow_left,
                    "top": overflow_top,
                    "right": overflow_right,
                    "bottom": overflow_bottom,
                    "width": overflow_left + overflow_right,
                    "height": overflow_top + overflow_bottom,
                },
                "scroll_range": {
                    "x": layout.scroll_max_x.get(id).copied().unwrap_or(0.0),
                    "y": layout.scroll_max_y.get(id).copied().unwrap_or(0.0),
                },
            }),
        );
    }
    json!({
        "rects": Value::Object(rects),
        "clips": Value::Object(clips),
        "diagnostics": Value::Object(diagnostics),
        "scroll_x": &layout.scroll_x,
        "scroll_y": &layout.scroll_y,
        "scroll_max_x": &layout.scroll_max_x,
        "scroll_max_y": &layout.scroll_max_y,
    })
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
        "container_scroll_x": &state.container_scroll_x,
        "container_scroll_y": &state.container_scroll_y,
        "dropdown_index": &state.dropdown_index,
        "dropdown_items_count": state.dropdown_items.iter().map(|(id, items)| (id.clone(), json!(items.len()))).collect::<Map<_, _>>(),
        "disabled": state.disabled.iter().cloned().collect::<Vec<_>>(),
        "focus_order": &state.focus_order,
        "focused": state.focused.as_deref(),
        "focus_t": &state.focus_t,
        "hovered": state.hovered.as_deref(),
        "hover_t": &state.hover_t,
        "checked_t": &state.checked_t,
        "active_t": &state.active_t,
        "open_t": &state.open_t,
        "selected_t": &state.selected_t,
        "expanded_t": &state.expanded_t,
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

fn theme_color_scheme(theme: &Theme) -> DgMediaColorScheme {
    let [r, g, b, _] = theme.background;
    let luminance = 0.2126 * linear_srgb(r) + 0.7152 * linear_srgb(g) + 0.0722 * linear_srgb(b);
    if luminance > 0.5 {
        DgMediaColorScheme::Light
    } else {
        DgMediaColorScheme::Dark
    }
}

fn winit_theme_color_scheme(theme: WinitTheme) -> DgMediaColorScheme {
    match theme {
        WinitTheme::Light => DgMediaColorScheme::Light,
        WinitTheme::Dark => DgMediaColorScheme::Dark,
    }
}

fn linear_srgb(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
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

fn set_widget_checked_prop(node: &mut WidgetNode, id: &str, checked: bool) -> bool {
    let Some(target) = find_widget_mut(node, id) else {
        return false;
    };
    if target.kind != WidgetKind::Checkbox {
        return false;
    }
    target.props.checked = Some(checked);
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
            | "align_items"
            | "align-items"
            | "align_self"
            | "align-self"
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
            | "margin_left"
            | "margin-left"
            | "margin_right"
            | "margin-right"
            | "margin_top"
            | "margin-top"
            | "margin_bottom"
            | "margin-bottom"
            | "gap"
            | "grid_auto_flow"
            | "grid-auto-flow"
            | "text_area_rows"
            | "text-area-rows"
            | "table_row_height"
            | "table-header-height"
            | "table_header_height"
            | "table-row-height"
            | "table_column_width"
            | "table-column-width"
            | "table_index_width"
            | "table-index-width"
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
    use crate::css_style::apply_stylesheets_to_tree;
    use crate::style::TransformStyle;
    use serde_json::json;

    #[test]
    fn command_batch_coalesces_scatter_updates() {
        let mut commands = vec![
            Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![1; 12],
                telemetry: None,
                colormap: "viridis".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                coalesce: true,
            },
            Command::DrainPythonTasks,
            Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![2; 12],
                telemetry: None,
                colormap: "turbo".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                coalesce: true,
            },
        ];

        coalesce_runtime_command_batch(&mut commands);

        assert_eq!(
            commands,
            vec![
                Command::DrainPythonTasks,
                Command::SetScatterPointsPacked {
                    id: "scatter".to_string(),
                    xyz: vec![2; 12],
                    telemetry: None,
                    colormap: "turbo".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    coalesce: true,
                },
            ]
        );
    }

    #[test]
    fn command_batch_coalescing_respects_debug_snapshot_barrier() {
        let mut commands = vec![
            Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![1; 12],
                telemetry: None,
                colormap: "viridis".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                coalesce: true,
            },
            Command::DebugSnapshot { request_id: 1 },
            Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![2; 12],
                telemetry: None,
                colormap: "turbo".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                coalesce: true,
            },
        ];

        coalesce_runtime_command_batch(&mut commands);

        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn winit_theme_maps_to_css_color_scheme() {
        assert_eq!(
            winit_theme_color_scheme(WinitTheme::Light),
            DgMediaColorScheme::Light
        );
        assert_eq!(
            winit_theme_color_scheme(WinitTheme::Dark),
            DgMediaColorScheme::Dark
        );
    }

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
            style_patch_dirty(json!({"text-area-rows": 5}).as_object().unwrap()),
            Dirty::Layout
        );
        assert_eq!(
            style_patch_dirty(json!({"scatter-point-size": 7}).as_object().unwrap()),
            Dirty::Visual
        );
        assert_eq!(
            style_patch_dirty(
                json!({"grid-auto-flow": "column dense"})
                    .as_object()
                    .unwrap()
            ),
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

        let snapshot = computed_styles_snapshot(Some(&tree), &store, None);
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

        let snapshot = computed_styles_snapshot(Some(&tree), &store, None);
        let transition = &snapshot["run"]["style"]["transition"];

        assert_eq!(transition["property"], json!(["background"]));
        assert_eq!(transition["duration_ms"], json!(180));
        assert_eq!(transition["delay_ms"], json!(25));
        assert_eq!(transition["timing_function"], json!("ease-out"));
    }

    #[test]
    fn cubic_bezier_transition_easing_solves_curve() {
        let linear = TransitionTimingFunction::CubicBezier {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let fast_out = TransitionTimingFunction::CubicBezier {
            x1: 0.16,
            y1: 1.0,
            x2: 0.3,
            y2: 1.0,
        };

        assert!((ease_transition(0.5, linear) - 0.5).abs() < 0.001);
        assert!(ease_transition(0.5, fast_out) > 0.85);
        assert_eq!(
            transition_timing_name(fast_out),
            "cubic-bezier(0.16, 1, 0.3, 1)"
        );
    }

    #[test]
    fn step_transition_easing_quantizes_progress() {
        let end = TransitionTimingFunction::Steps {
            count: 4,
            position: StepPosition::End,
        };
        let start = TransitionTimingFunction::Steps {
            count: 4,
            position: StepPosition::Start,
        };

        assert_eq!(ease_transition(0.24, end), 0.0);
        assert_eq!(ease_transition(0.25, end), 0.25);
        assert_eq!(ease_transition(0.01, start), 0.25);
        assert_eq!(ease_transition(0.99, start), 1.0);
        assert_eq!(transition_timing_name(start), "steps(4, start)");
    }

    #[test]
    fn fractional_animation_iteration_count_resolves_final_progress() {
        let linear = TransitionTimingFunction::Linear;

        assert_eq!(
            animation_final_progress(1.5, AnimationDirection::Normal, linear),
            0.5
        );
        assert_eq!(
            animation_final_progress(2.0, AnimationDirection::Alternate, linear),
            0.0
        );
        assert_eq!(
            animation_final_progress(3.0, AnimationDirection::Alternate, linear),
            1.0
        );
        assert_eq!(
            animation_final_progress(2.5, AnimationDirection::AlternateReverse, linear),
            0.5
        );
    }

    #[test]
    fn negative_animation_delay_advances_elapsed_time() {
        assert_eq!(
            animation_elapsed_after_delay(Duration::from_millis(100), 250),
            None
        );
        assert_eq!(
            animation_elapsed_after_delay(Duration::from_millis(300), 250),
            Some(Duration::from_millis(50))
        );
        assert_eq!(
            animation_elapsed_after_delay(Duration::from_millis(100), -250),
            Some(Duration::from_millis(350))
        );
    }

    #[test]
    fn keyframe_animation_visual_interpolates_transform_and_opacity() {
        let keyframes = DgKeyframes {
            name: "pulse".to_string(),
            frames: vec![
                crate::css_style::DgKeyframe {
                    offset: 0.0,
                    visual: VisualStyle {
                        opacity: Some(0.5),
                        transform: Some(TransformStyle {
                            scale_x: 0.9,
                            scale_y: 0.9,
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                },
                crate::css_style::DgKeyframe {
                    offset: 1.0,
                    visual: VisualStyle {
                        opacity: Some(1.0),
                        transform: Some(TransformStyle {
                            scale_x: 1.1,
                            scale_y: 1.1,
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                },
            ],
        };

        let visual = animation_visual_at(&keyframes, 0.25, &Theme::dark()).unwrap();
        assert!((visual.opacity.unwrap() - 0.625).abs() < 0.001);
        let transform = visual.transform.unwrap();
        assert!((transform.scale_x - 0.95).abs() < 0.001);
        assert!((transform.scale_y - 0.95).abs() < 0.001);
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
        let mut focus_transitions = HashMap::from([(
            "field".to_string(),
            HoverTransition {
                start: Instant::now(),
                duration: Duration::from_millis(120),
                delay: Duration::ZERO,
                from: 0.0,
                to: 1.0,
                timing: TransitionTimingFunction::EaseOut,
            },
        )]);
        let mut checked_transitions = HashMap::from([(
            "enabled".to_string(),
            HoverTransition {
                start: Instant::now(),
                duration: Duration::from_millis(120),
                delay: Duration::ZERO,
                from: 0.0,
                to: 1.0,
                timing: TransitionTimingFunction::EaseOut,
            },
        )]);
        let mut active_transitions = HashMap::from([(
            "submit".to_string(),
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
        let mut expanded_transitions = HashMap::from([(
            "advanced".to_string(),
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
        state.focus_t.insert("field".to_string(), 0.5);
        state.checked_t.insert("enabled".to_string(), 0.5);
        state.active_t.insert("submit".to_string(), 0.5);
        state.open_t.insert("mode".to_string(), 0.5);
        state.selected_t.insert("tab-a".to_string(), 0.5);
        state.expanded_t.insert("advanced".to_string(), 0.5);
        let mut state = Some(state);

        assert!(clear_style_transition_state(
            &mut transitions,
            &mut focus_transitions,
            &mut checked_transitions,
            &mut active_transitions,
            &mut open_transitions,
            &mut selected_transitions,
            &mut expanded_transitions,
            &mut state
        ));
        assert!(transitions.is_empty());
        assert!(focus_transitions.is_empty());
        assert!(checked_transitions.is_empty());
        assert!(active_transitions.is_empty());
        assert!(open_transitions.is_empty());
        assert!(selected_transitions.is_empty());
        assert!(expanded_transitions.is_empty());
        assert!(state.as_ref().unwrap().hover_t.is_empty());
        assert!(state.as_ref().unwrap().focus_t.is_empty());
        assert!(state.as_ref().unwrap().checked_t.is_empty());
        assert!(state.as_ref().unwrap().active_t.is_empty());
        assert!(state.as_ref().unwrap().open_t.is_empty());
        assert!(state.as_ref().unwrap().selected_t.is_empty());
        assert!(state.as_ref().unwrap().expanded_t.is_empty());
        assert!(!clear_style_transition_state(
            &mut transitions,
            &mut focus_transitions,
            &mut checked_transitions,
            &mut active_transitions,
            &mut open_transitions,
            &mut selected_transitions,
            &mut expanded_transitions,
            &mut state
        ));
    }

    #[test]
    fn scrollbar_drag_maps_pointer_to_scroll_range() {
        let hit = PanelScrollbarHit {
            widget_id: "panel".to_string(),
            axis: PanelScrollbarAxis::Vertical,
            geometry: PanelScrollbarAxisGeometry {
                track: crate::layout::Rect {
                    x: 90.0,
                    y: 10.0,
                    w: 4.0,
                    h: 100.0,
                },
                thumb: crate::layout::Rect {
                    x: 90.0,
                    y: 10.0,
                    w: 4.0,
                    h: 25.0,
                },
                max_scroll: 300.0,
            },
            on_thumb: true,
        };
        let drag = ScrollbarDrag::new(hit, [92.0, 20.0]);

        assert_eq!(drag.compute_scroll([92.0, 20.0]), 0.0);
        assert_eq!(drag.compute_scroll([92.0, 95.0]), 300.0);
    }

    #[test]
    fn scroll_keyboard_destination_pages_and_jumps_axes() {
        let target = ScrollContainerKeyboardTarget {
            id: "panel".to_string(),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 100.0,
            },
            current_x: 30.0,
            current_y: 20.0,
            max_x: 400.0,
            max_y: 300.0,
        };

        assert_eq!(
            scroll_keyboard_destination(&target, ScrollKeyboardCommand::PageForward, false),
            Some((PanelScrollbarAxis::Vertical, 105.0))
        );
        assert_eq!(
            scroll_keyboard_destination(&target, ScrollKeyboardCommand::PageBackward, false),
            Some((PanelScrollbarAxis::Vertical, 0.0))
        );
        assert_eq!(
            scroll_keyboard_destination(&target, ScrollKeyboardCommand::End, false),
            Some((PanelScrollbarAxis::Vertical, 300.0))
        );
        assert_eq!(
            scroll_keyboard_destination(&target, ScrollKeyboardCommand::PageForward, true),
            Some((PanelScrollbarAxis::Horizontal, 200.0))
        );
    }

    #[test]
    fn scroll_keyboard_destination_falls_back_to_horizontal_when_needed() {
        let target = ScrollContainerKeyboardTarget {
            id: "strip".to_string(),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 80.0,
            },
            current_x: 10.0,
            current_y: 0.0,
            max_x: 250.0,
            max_y: 0.0,
        };

        assert_eq!(
            scroll_keyboard_destination(&target, ScrollKeyboardCommand::PageForward, false),
            Some((PanelScrollbarAxis::Horizontal, 112.0))
        );
        assert_eq!(
            scroll_keyboard_destination(&target, ScrollKeyboardCommand::End, false),
            Some((PanelScrollbarAxis::Horizontal, 250.0))
        );
    }

    #[test]
    fn wheel_target_prefers_nested_scroll_panel_over_root_scroller() {
        let buttons: Vec<_> = (1..=10)
            .map(|index| {
                json!({
                    "id": format!("button-{index}"),
                    "type": "button",
                    "props": {"text": format!("Scrollable row {index}")},
                    "style": {"height": 30}
                })
            })
            .collect();
        let mut children = vec![json!({
            "id": "intro",
            "type": "label",
            "props": {"text": "The title should stay above the scrollable body."}
        })];
        children.extend(buttons);
        children.push(json!({
            "id": "pass",
            "type": "label",
            "props": {"text": "PASS: final row can scroll fully into view."}
        }));

        let tree = crate::document::parse_widget_node(&json!({
            "id": "window",
            "type": "window",
            "children": [{
                "id": "root",
                "type": "v_layout",
                "style": {
                    "height": 400,
                    "overflow_y": "auto",
                    "gap": 12
                },
                "children": [
                    {
                        "id": "before",
                        "type": "spacer",
                        "props": {"height": 260}
                    },
                    {
                        "id": "scroll-panel",
                        "type": "panel",
                        "props": {"title": "Scrollable titled panel"},
                        "style": {
                            "height": 250,
                            "overflow_y": "auto",
                            "overflow_x": "hidden",
                            "padding": 14,
                            "padding_right": 26,
                            "padding_bottom": 22,
                            "gap": 10
                        },
                        "children": children
                    },
                    {
                        "id": "after",
                        "type": "spacer",
                        "props": {"height": 260}
                    }
                ]
            }]
        }))
        .unwrap();
        for root_scroll in [0.0, 140.0, 250.0] {
            let mut state = WidgetState::from_tree(&tree);
            state
                .container_scroll_y
                .insert("root".to_string(), root_scroll);
            let layout = crate::layout::compute_layout(
                &tree,
                700.0,
                500.0,
                1.0,
                &Theme::dark(),
                Some(&state),
            );
            let panel = layout.rects.get("scroll-panel").unwrap();
            let panel_clip = layout.clips.get("scroll-panel").unwrap();
            let pos = [
                panel_clip.x + 40.0,
                (panel_clip.y + 100.0).min(panel_clip.y + panel_clip.h - 4.0),
            ];

            assert!(layout.scroll_max_y.get("root").copied().unwrap_or(0.0) > 0.0);
            assert!(
                layout
                    .scroll_max_y
                    .get("scroll-panel")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
            );
            assert_eq!(
                scroll_container_at_pos(&tree, &layout, &state, pos),
                Some("scroll-panel".to_string()),
                "root_scroll={root_scroll} panel={panel:?} clip={panel_clip:?} pos={pos:?}"
            );
        }
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

        let snapshot = computed_styles_snapshot(Some(&tree), &store, None);
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
    fn decode_scatter_points_invalid_payload_preserves_old_points() {
        // A truncated XyzF32V0 payload (5 bytes — not a multiple of 12) must fail decode.
        // The old runtime.points should be untouched and the error propagated.
        let sentinel = scatter::PointInstance {
            position: [9.0, 9.0, 9.0],
            color: [1.0, 0.0, 0.0],
            alpha: 1.0,
            size: 5.0,
        };
        let old_pts: Vec<scatter::PointInstance> = vec![sentinel];
        let bad_bytes: Vec<u8> = vec![0u8; 5]; // 5 bytes — not a multiple of 12
        let mut decoded: Vec<scatter::PointInstance> = Vec::new();
        let result = decode_scatter_points_bytes_into_colormap(&bad_bytes, &mut decoded, "viridis");
        assert!(result.is_err(), "truncated payload must return an error");
        // Simulate the new policy: on error we do NOT modify old_pts.
        if result.is_err() {
            // old_pts intentionally not touched
        }
        assert_eq!(
            old_pts.len(),
            1,
            "old points must be preserved on decode error"
        );
        assert_eq!(old_pts[0].position, [9.0, 9.0, 9.0]);
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
    fn set_widget_checked_prop_updates_retained_checkbox_state() {
        let mut root = document::parse_widget_node(&json!({
            "id": "window",
            "type": "window",
            "props": {},
            "children": [{
                "id": "enabled",
                "type": "checkbox",
                "props": {"label": "Enabled", "checked": false}
            }]
        }))
        .unwrap();

        assert!(set_widget_checked_prop(&mut root, "enabled", true));
        let checkbox = find_widget_mut(&mut root, "enabled").unwrap();
        assert_eq!(checkbox.props.checked, Some(true));
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
    styles_dirty: bool,
    last_style_media: Option<DgMediaEnvironment>,
    scale_factor: f32,
    platform_color_scheme: Option<DgMediaColorScheme>,
    /// Per-widget scatter state keyed by widget id.
    scatters: HashMap<String, ScatterRuntime>,
    /// Visible scatter widget ids in paint order (updated each apply_layout).
    visible_scatter_order: Vec<String>,
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
    focus_transitions: HashMap<String, HoverTransition>,
    focus_state_snapshot: HashSet<String>,
    checked_transitions: HashMap<String, HoverTransition>,
    checked_state_snapshot: HashSet<String>,
    active_transitions: HashMap<String, HoverTransition>,
    active_state_snapshot: HashSet<String>,
    open_transitions: HashMap<String, HoverTransition>,
    open_state_snapshot: HashSet<String>,
    selected_transitions: HashMap<String, HoverTransition>,
    selected_state_snapshot: HashSet<String>,
    expanded_transitions: HashMap<String, HoverTransition>,
    expanded_state_snapshot: HashSet<String>,
    animation_epoch: Instant,
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
    last_bounds_ms: f64,
    last_upload_ms: f64,
    last_primary_upload_ms: f64,
    last_lod_ms: f64,
    last_grid_ms: f64,
    last_overlay_ms: f64,
    last_total_native_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScatterPayloadStatus {
    Empty,
    Ok,
    /// Payload decoded successfully but every point position was non-finite.
    AllNonFinite,
    DecodeError(String),
}

impl Default for ScatterPayloadStatus {
    fn default() -> Self {
        Self::Empty
    }
}

/// Per-widget scatter runtime state.
struct ScatterRuntime {
    widget: ScatterWidget,
    /// CPU copy of point instances used for picking.
    points: Vec<PointInstance>,
    metrics: ScatterMetrics,
    /// True after the first successful data load; prevents auto-fit on live updates.
    fitted_once: bool,
    data_min: glam::Vec3,
    data_max: glam::Vec3,
    payload_format: ScatterPayloadFormat,
    payload_status: ScatterPayloadStatus,
    /// Per-point tooltip text for the primary buffer (actor_id == 0).
    primary_hover_meta: Vec<String>,
    /// Column names used as coordinate row labels in hover tooltips (x, y, z order).
    tooltip_axis_labels: [String; 3],
    /// When true, hover movement triggers a point-pick and a tooltip overlay.
    hover_tooltip_enabled: bool,
    /// Lazily-built screen-space pick cache for the primary point buffer.
    primary_pick_cache: Option<scatter::ScreenPickCache>,
}

impl ScatterRuntime {
    /// Bounds of actor 0 (legacy single scene) merged with all visible extra actors.
    fn merged_bounds(&self) -> (glam::Vec3, glam::Vec3) {
        let mut mn = self.data_min;
        let mut mx = self.data_max;
        if let Some((emn, emx)) = self.widget.merged_extra_bounds() {
            mn = mn.min(emn);
            mx = mx.max(emx);
        }
        if let Some((mmn, mmx)) = self.widget.merged_mesh_bounds() {
            mn = mn.min(mmn);
            mx = mx.max(mmx);
        }
        (mn, mx)
    }

    /// Pick the closest point across the primary buffer and all visible extra actors,
    /// using lazily-built per-actor screen-space grid caches to avoid O(N) scans.
    ///
    /// Caches are rebuilt automatically when the view–projection matrix or viewport
    /// size has changed since the last query.  Data changes (upload, stream, clear)
    /// must invalidate the relevant cache by setting it to `None`.
    ///
    /// Returns `(actor_id, index, point)` where `actor_id == 0` means primary buffer.
    fn pick_all_actors_cached(
        &mut self,
        x: f32,
        y: f32,
        radius_px: f32,
    ) -> Option<(u32, usize, PointInstance)> {
        if !self.widget.contains_point(x, y) || self.widget.width == 0 || self.widget.height == 0 {
            return None;
        }
        let local_x = x - self.widget.offset[0];
        let local_y = y - self.widget.offset[1];
        let view_proj = self.widget.camera.view_proj();
        let w = self.widget.width;
        let h = self.widget.height;

        let pso = self.widget.point_size_override;

        // Rebuild primary cache if stale or missing.
        let primary_stale = self
            .primary_pick_cache
            .as_ref()
            .map(|c| c.is_stale(&view_proj, w, h, pso))
            .unwrap_or(true);
        if primary_stale {
            let new_cache = scatter::ScreenPickCache::build(&self.points, view_proj, w, h, pso);
            self.primary_pick_cache = Some(new_cache);
        }

        // Rebuild stale actor caches.
        let actor_ids: Vec<u32> = self.widget.extra_actors.keys().copied().collect();
        for actor_id in &actor_ids {
            if let Some(actor) = self.widget.extra_actors.get_mut(actor_id) {
                if !actor.visible {
                    continue;
                }
                let stale = actor
                    .pick_cache
                    .as_ref()
                    .map(|c| c.is_stale(&view_proj, w, h, pso))
                    .unwrap_or(true);
                if stale {
                    let new_cache = {
                        let live_pts = &actor.points[..actor.point_count as usize];
                        scatter::ScreenPickCache::build(live_pts, view_proj, w, h, pso)
                    };
                    actor.pick_cache = Some(new_cache);
                }
            }
        }

        // (actor_id, index, point, dist2, depth)
        let mut best: Option<(u32, usize, PointInstance, f32, f32)> = None;
        let mut update = |actor_id: u32, idx: usize, pt: PointInstance, dist2: f32, depth: f32| {
            // Mirror pick_point_all_actors: prefer closer dist2, break ties toward nearer depth.
            let take = match best {
                None => true,
                Some((_, _, _, bd, bz))
                    if dist2 < bd || ((bd - dist2).abs() <= f32::EPSILON && depth < bz) =>
                {
                    true
                }
                _ => false,
            };
            if take {
                best = Some((actor_id, idx, pt, dist2, depth));
            }
        };

        // Query primary buffer via cache, expanding radius for large points.
        let primary_query_radius = {
            let cache = self.primary_pick_cache.as_ref().unwrap();
            radius_px.max(cache.max_point_size * 0.75)
        };
        let primary_cands = {
            let cache = self.primary_pick_cache.as_ref().unwrap();
            cache.candidates(local_x, local_y, primary_query_radius)
        };
        for idx_u32 in primary_cands {
            let idx = idx_u32 as usize;
            if idx >= self.points.len() {
                continue;
            }
            let pt = self.points[idx];
            if let Some((d2, z)) = self.widget.pick_distance(pt, local_x, local_y, radius_px) {
                update(0, idx, pt, d2, z);
            }
        }

        // Query extra actors via their caches, expanding radius for large points.
        for actor_id in actor_ids {
            let actor = match self.widget.extra_actors.get(&actor_id) {
                Some(a) if a.visible => a,
                _ => continue,
            };
            let (actor_query_radius, cands) = {
                let cache = match actor.pick_cache.as_ref() {
                    Some(c) => c,
                    None => continue,
                };
                let qr = radius_px.max(cache.max_point_size * 0.75);
                (qr, cache.candidates(local_x, local_y, qr))
            };
            let _ = actor_query_radius; // used to compute cands above
            let live_pts = &actor.points[..actor.point_count as usize];
            for idx_u32 in cands {
                let idx = idx_u32 as usize;
                if idx >= live_pts.len() {
                    continue;
                }
                let pt = live_pts[idx];
                if let Some((d2, z)) = self.widget.pick_distance(pt, local_x, local_y, radius_px) {
                    update(actor_id, idx, pt, d2, z);
                }
            }
        }

        best.map(|(actor_id, idx, pt, _, _)| (actor_id, idx, pt))
    }
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
        TransitionTimingFunction::Steps { count, position } => steps_transition(t, count, position),
        TransitionTimingFunction::CubicBezier { x1, y1, x2, y2 } => {
            cubic_bezier_transition(t, x1, y1, x2, y2)
        }
    }
}

fn steps_transition(t: f32, count: u32, position: StepPosition) -> f32 {
    if t <= 0.0 {
        return match position {
            StepPosition::Start => 1.0 / count.max(1) as f32,
            StepPosition::End => 0.0,
        };
    }
    if t >= 1.0 {
        return 1.0;
    }
    let count = count.max(1) as f32;
    match position {
        StepPosition::Start => (t * count).ceil() / count,
        StepPosition::End => (t * count).floor() / count,
    }
    .clamp(0.0, 1.0)
}

fn cubic_bezier_transition(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

    let mut parameter = t;
    for _ in 0..8 {
        let x = cubic_bezier_axis(parameter, x1, x2) - t;
        if x.abs() < 0.000_01 {
            return cubic_bezier_axis(parameter, y1, y2).clamp(0.0, 1.0);
        }
        let derivative = cubic_bezier_axis_derivative(parameter, x1, x2);
        if derivative.abs() < 0.000_001 {
            break;
        }
        let next = parameter - x / derivative;
        if !(0.0..=1.0).contains(&next) {
            break;
        }
        parameter = next;
    }

    let mut lower = 0.0;
    let mut upper = 1.0;
    parameter = t;
    for _ in 0..16 {
        let x = cubic_bezier_axis(parameter, x1, x2);
        if (x - t).abs() < 0.000_01 {
            break;
        }
        if x < t {
            lower = parameter;
        } else {
            upper = parameter;
        }
        parameter = (lower + upper) * 0.5;
    }
    cubic_bezier_axis(parameter, y1, y2).clamp(0.0, 1.0)
}

fn cubic_bezier_axis(t: f32, p1: f32, p2: f32) -> f32 {
    let c = 3.0 * p1;
    let b = 3.0 * (p2 - p1) - c;
    let a = 1.0 - c - b;
    ((a * t + b) * t + c) * t
}

fn cubic_bezier_axis_derivative(t: f32, p1: f32, p2: f32) -> f32 {
    let c = 3.0 * p1;
    let b = 3.0 * (p2 - p1) - c;
    let a = 1.0 - c - b;
    (3.0 * a * t + 2.0 * b) * t + c
}

fn animation_config(
    style: &AnimationStyle,
) -> Option<(
    String,
    Duration,
    i64,
    TransitionTimingFunction,
    AnimationIterationCount,
    AnimationDirection,
    AnimationFillMode,
    AnimationPlayState,
)> {
    let name = style.name.clone()?;
    let duration_ms = style.duration_ms?;
    if duration_ms == 0 {
        return None;
    }
    Some((
        name,
        Duration::from_millis(duration_ms),
        style.delay_ms.unwrap_or(0),
        style
            .timing_function
            .unwrap_or(TransitionTimingFunction::Ease),
        style
            .iteration_count
            .unwrap_or(AnimationIterationCount::One),
        style.direction.unwrap_or(AnimationDirection::Normal),
        style.fill_mode.unwrap_or(AnimationFillMode::None),
        style.play_state.unwrap_or(AnimationPlayState::Running),
    ))
}

fn animation_elapsed_after_delay(elapsed: Duration, delay_ms: i64) -> Option<Duration> {
    if delay_ms >= 0 {
        let delay = Duration::from_millis(delay_ms as u64);
        if elapsed >= delay {
            Some(elapsed - delay)
        } else {
            None
        }
    } else {
        Some(elapsed + Duration::from_millis(delay_ms.unsigned_abs()))
    }
}

fn animation_iteration_total(count: AnimationIterationCount) -> Option<f32> {
    match count {
        AnimationIterationCount::One => Some(1.0),
        AnimationIterationCount::Count(value) => Some(value.max(0.0001)),
        AnimationIterationCount::Infinite => None,
    }
}

fn animation_directed_progress(t: f32, iteration: u32, direction: AnimationDirection) -> f32 {
    match direction {
        AnimationDirection::Normal => t,
        AnimationDirection::Reverse => 1.0 - t,
        AnimationDirection::Alternate => {
            if iteration % 2 == 0 {
                t
            } else {
                1.0 - t
            }
        }
        AnimationDirection::AlternateReverse => {
            if iteration % 2 == 0 {
                1.0 - t
            } else {
                t
            }
        }
    }
}

fn animation_final_progress(
    total_iterations: f32,
    direction: AnimationDirection,
    timing: TransitionTimingFunction,
) -> f32 {
    let total = total_iterations.max(0.0001);
    let whole = total.floor();
    let fract = total - whole;
    let (iteration, local) = if fract <= 0.0001 {
        ((whole as u32).saturating_sub(1), 1.0)
    } else {
        (whole as u32, fract)
    };
    ease_transition(
        animation_directed_progress(local, iteration, direction),
        timing,
    )
}

fn animation_visual_at(
    keyframes: &DgKeyframes,
    progress: f32,
    theme: &Theme,
) -> Option<VisualStyle> {
    let frames = &keyframes.frames;
    let first = frames.first()?;
    let last = frames.last()?;
    if progress <= first.offset {
        return Some(first.visual.clone());
    }
    if progress >= last.offset {
        return Some(last.visual.clone());
    }
    for pair in frames.windows(2) {
        let from = &pair[0];
        let to = &pair[1];
        if progress >= from.offset && progress <= to.offset {
            let span = (to.offset - from.offset).max(0.0001);
            let local = ((progress - from.offset) / span).clamp(0.0, 1.0);
            return Some(interpolate_visual_style(
                &from.visual,
                &to.visual,
                &to.visual,
                local,
                theme,
                None,
            ));
        }
    }
    Some(last.visual.clone())
}

fn collect_animation_visuals(
    node: &WidgetNode,
    keyframes: &BTreeMap<String, DgKeyframes>,
    elapsed: Duration,
    theme: &Theme,
    out: &mut HashMap<String, VisualStyle>,
    active: &mut bool,
) {
    if let Some(visual) = node_animation_visual(node, keyframes, elapsed, theme, active) {
        out.insert(node.id.clone(), visual);
    }
    for child in &node.children {
        collect_animation_visuals(child, keyframes, elapsed, theme, out, active);
    }
}

fn node_animation_visual(
    node: &WidgetNode,
    keyframes: &BTreeMap<String, DgKeyframes>,
    elapsed: Duration,
    theme: &Theme,
    active: &mut bool,
) -> Option<VisualStyle> {
    let (name, duration, delay, timing, iteration_count, direction, fill_mode, play_state) =
        animation_config(&node.style.animation)?;
    let keyframes = keyframes.get(&name)?;
    if play_state == AnimationPlayState::Paused {
        return animation_visual_at(
            keyframes,
            animation_directed_progress(0.0, 0, direction),
            theme,
        );
    }
    let Some(active_elapsed) = animation_elapsed_after_delay(elapsed, delay) else {
        *active = true;
        return match fill_mode {
            AnimationFillMode::Backwards | AnimationFillMode::Both => animation_visual_at(
                keyframes,
                animation_directed_progress(0.0, 0, direction),
                theme,
            ),
            AnimationFillMode::None | AnimationFillMode::Forwards => None,
        };
    };

    let duration_secs = duration.as_secs_f32().max(0.0001);
    let raw_iterations = active_elapsed.as_secs_f32() / duration_secs;
    let iteration = raw_iterations.floor() as u32;
    if let Some(total) = animation_iteration_total(iteration_count) {
        if raw_iterations >= total {
            return match fill_mode {
                AnimationFillMode::Forwards | AnimationFillMode::Both => animation_visual_at(
                    keyframes,
                    animation_final_progress(total, direction, timing),
                    theme,
                ),
                AnimationFillMode::None | AnimationFillMode::Backwards => None,
            };
        }
    }

    *active = true;
    let local = raw_iterations.fract();
    let directed = animation_directed_progress(local, iteration, direction);
    let eased = ease_transition(directed, timing);
    animation_visual_at(keyframes, eased, theme)
}

fn clear_style_transition_state(
    hover_transitions: &mut HashMap<String, HoverTransition>,
    focus_transitions: &mut HashMap<String, HoverTransition>,
    checked_transitions: &mut HashMap<String, HoverTransition>,
    active_transitions: &mut HashMap<String, HoverTransition>,
    open_transitions: &mut HashMap<String, HoverTransition>,
    selected_transitions: &mut HashMap<String, HoverTransition>,
    expanded_transitions: &mut HashMap<String, HoverTransition>,
    widget_state: &mut Option<WidgetState>,
) -> bool {
    let had_transitions = !hover_transitions.is_empty()
        || !focus_transitions.is_empty()
        || !checked_transitions.is_empty()
        || !active_transitions.is_empty()
        || !open_transitions.is_empty()
        || !selected_transitions.is_empty()
        || !expanded_transitions.is_empty();
    hover_transitions.clear();
    focus_transitions.clear();
    checked_transitions.clear();
    active_transitions.clear();
    open_transitions.clear();
    selected_transitions.clear();
    expanded_transitions.clear();
    let had_progress = widget_state.as_mut().is_some_and(|state| {
        let had_hover = !std::mem::take(&mut state.hover_t).is_empty();
        let had_focus = !std::mem::take(&mut state.focus_t).is_empty();
        let had_checked = !std::mem::take(&mut state.checked_t).is_empty();
        let had_active = !std::mem::take(&mut state.active_t).is_empty();
        let had_open = !std::mem::take(&mut state.open_t).is_empty();
        let had_selected = !std::mem::take(&mut state.selected_t).is_empty();
        let had_expanded = !std::mem::take(&mut state.expanded_t).is_empty();
        had_hover
            || had_focus
            || had_checked
            || had_active
            || had_open
            || had_selected
            || had_expanded
    });
    had_transitions || had_progress
}

fn collect_focused_widget_ids(
    _node: &WidgetNode,
    state: Option<&WidgetState>,
    out: &mut HashSet<String>,
) {
    if let Some(id) = state.and_then(|state| state.focused.as_ref()) {
        out.insert(id.clone());
    }
}

fn collect_active_widget_ids(
    _node: &WidgetNode,
    state: Option<&WidgetState>,
    out: &mut HashSet<String>,
) {
    if let Some(id) = state.and_then(|state| state.pressed.as_ref()) {
        out.insert(id.clone());
    }
}

fn collect_checked_widget_ids(
    node: &WidgetNode,
    state: Option<&WidgetState>,
    out: &mut HashSet<String>,
) {
    if state.is_some_and(|state| state.checked.get(&node.id).copied().unwrap_or(false)) {
        out.insert(node.id.clone());
    }
    for child in &node.children {
        collect_checked_widget_ids(child, state, out);
    }
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

fn collect_expanded_widget_ids(
    node: &WidgetNode,
    state: Option<&WidgetState>,
    out: &mut HashSet<String>,
) {
    if state.is_some_and(|state| state.is_expanded_widget(&node.id)) {
        out.insert(node.id.clone());
    }
    for child in &node.children {
        collect_expanded_widget_ids(child, state, out);
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

        // Build one ScatterRuntime per Scatter3D node in the tree.
        let mut scatters: HashMap<String, ScatterRuntime> = HashMap::new();
        let mut total_upload_ms = 0.0_f64;
        if let Some(tree) = &spec.widget_tree {
            let mut scatter_ids: Vec<String> = Vec::new();
            collect_all_scatter_ids(tree, &mut scatter_ids);
            for scatter_id in scatter_ids {
                let node = find_widget(tree, &scatter_id);
                let (colormap, data_b64, data_format, startup_chrome) = node
                    .map(|n| {
                        let chrome = scatter_chrome_from_props(&n.props);
                        (
                            n.props
                                .scatter_colormap
                                .clone()
                                .unwrap_or_else(|| "viridis".to_string()),
                            n.props.scatter_data_b64.clone(),
                            n.props.scatter_data_format,
                            chrome,
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            "viridis".to_string(),
                            None,
                            ScatterPayloadFormat::default(),
                            scatter::ScatterChromeState::default(),
                        )
                    });

                let mut widget = ScatterWidget::new(&device, config.format, width, height);
                let t0 = Instant::now();
                let (mut pts, mut status) = if let Some(b64) = data_b64 {
                    let decoded_bytes = BASE64.decode(&b64);
                    match decoded_bytes {
                        Err(e) => {
                            let msg = format!("scatter base64 decode: {e}");
                            eprintln!("DragonGUI: {msg}");
                            (Vec::new(), ScatterPayloadStatus::DecodeError(msg))
                        }
                        Ok(bytes) => {
                            let mut pts = Vec::new();
                            let result = match data_format {
                                ScatterPayloadFormat::XyzF32V0 => {
                                    decode_scatter_points_bytes_into_colormap(
                                        &bytes, &mut pts, &colormap,
                                    )
                                }
                                ScatterPayloadFormat::PointInstanceV1 => {
                                    decode_scatter_points_v1(&bytes, &mut pts)
                                }
                            };
                            match result {
                                Err(e) => {
                                    let msg = e.to_string();
                                    eprintln!("DragonGUI: {msg}");
                                    (Vec::new(), ScatterPayloadStatus::DecodeError(msg))
                                }
                                Ok(_) => (pts, ScatterPayloadStatus::Ok),
                            }
                        }
                    }
                } else {
                    (Vec::new(), ScatterPayloadStatus::Empty)
                };

                let maybe_bounds = compute_scatter_bounds(&pts);
                if maybe_bounds.is_none() && !pts.is_empty() {
                    status = ScatterPayloadStatus::AllNonFinite;
                    pts.clear();
                }
                let (data_min, data_max) =
                    maybe_bounds.unwrap_or((glam::Vec3::ZERO, glam::Vec3::ZERO));
                if !pts.is_empty() {
                    widget.set_points(&device, &queue, &pts);
                    widget.fit_to_bounds(data_min, data_max, &queue);
                } else {
                    widget.update_camera(&queue);
                }
                widget.set_chrome(startup_chrome);
                widget.refresh_grid(data_min, data_max, &device, &queue);
                widget.refresh_overlays(&device, &queue);
                total_upload_ms += t0.elapsed().as_secs_f64() * 1000.0;

                scatters.insert(
                    scatter_id,
                    ScatterRuntime {
                        widget,
                        points: pts,
                        metrics: ScatterMetrics::default(),
                        fitted_once: matches!(status, ScatterPayloadStatus::Ok),
                        data_min,
                        data_max,
                        payload_format: data_format,
                        payload_status: status,
                        primary_hover_meta: Vec::new(),
                        tooltip_axis_labels: ["x".to_string(), "y".to_string(), "z".to_string()],
                        hover_tooltip_enabled: true,
                        primary_pick_cache: None,
                    },
                );
            }
        }
        let upload_ms = total_upload_ms;

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
        let mut focus_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_focused_widget_ids(tree, widget_state.as_ref(), &mut focus_state_snapshot);
        }
        let mut checked_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_checked_widget_ids(tree, widget_state.as_ref(), &mut checked_state_snapshot);
        }
        let mut active_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_active_widget_ids(tree, widget_state.as_ref(), &mut active_state_snapshot);
        }
        let mut open_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_open_widget_ids(tree, widget_state.as_ref(), &mut open_state_snapshot);
        }
        let mut selected_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_selected_widget_ids(tree, widget_state.as_ref(), &mut selected_state_snapshot);
        }
        let mut expanded_state_snapshot = HashSet::new();
        if let Some(tree) = &spec.widget_tree {
            collect_expanded_widget_ids(tree, widget_state.as_ref(), &mut expanded_state_snapshot);
        }
        let mut widget_kinds = HashMap::new();
        if let Some(tree) = &spec.widget_tree {
            collect_widget_kinds(tree, &mut widget_kinds);
        }

        let mut state = Self {
            surface,
            device,
            queue,
            config,
            _depth_texture: depth_texture,
            depth_view,
            theme,
            stylesheets: spec.stylesheets,
            styles_dirty: true,
            last_style_media: None,
            scale_factor,
            platform_color_scheme: window.theme().map(winit_theme_color_scheme),
            scatters,
            visible_scatter_order: Vec::new(),
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
            focus_transitions: HashMap::new(),
            focus_state_snapshot,
            checked_transitions: HashMap::new(),
            checked_state_snapshot,
            active_transitions: HashMap::new(),
            active_state_snapshot,
            open_transitions: HashMap::new(),
            open_state_snapshot,
            selected_transitions: HashMap::new(),
            selected_state_snapshot,
            expanded_transitions: HashMap::new(),
            expanded_state_snapshot,
            animation_epoch: Instant::now(),
        };

        state.apply_layout();

        Ok((state, upload_ms))
    }

    fn media_environment(&self) -> DgMediaEnvironment {
        let scale_factor = self.scale_factor.max(0.001);
        DgMediaEnvironment::with_color_scheme(
            self.config.width as f32 / scale_factor,
            self.config.height as f32 / scale_factor,
            scale_factor,
            DgMediaColorGamut::Srgb,
            DgMediaPointer::Fine,
            DgMediaPointer::Fine,
            DgMediaHover::Hover,
            DgMediaHover::Hover,
            false,
            self.platform_color_scheme
                .unwrap_or_else(|| theme_color_scheme(&self.theme)),
        )
    }

    fn reapply_stylesheets_for_current_viewport(&mut self) {
        let media = self.media_environment();
        if !self.styles_dirty && self.last_style_media == Some(media) {
            return;
        }
        if let Some(tree) = &mut self.widget_tree {
            apply_stylesheets_to_tree_for_media(tree, &mut self.stylesheets, media);
        }
        self.styles_dirty = false;
        self.last_style_media = Some(media);
    }

    fn mark_styles_dirty(&mut self) {
        self.styles_dirty = true;
    }

    fn set_platform_color_scheme(&mut self, scheme: DgMediaColorScheme) {
        if self.platform_color_scheme == Some(scheme) {
            return;
        }
        self.platform_color_scheme = Some(scheme);
        self.mark_styles_dirty();
        self.apply_layout();
    }

    /// Recompute layout and push scatter viewport + primitives + text to GPU.
    fn apply_layout(&mut self) {
        self.reapply_stylesheets_for_current_viewport();
        self.sync_focus_transitions();
        self.sync_checked_transitions();
        self.sync_active_transitions();
        self.sync_open_transitions();
        self.sync_selected_transitions();
        self.sync_expanded_transitions();
        let media = self.media_environment();
        // Destructure to get separate borrows of each field.
        let WgpuState {
            widget_tree,
            current_layout,
            widget_state,
            resources,
            primitives,
            images,
            text,
            scatters,
            visible_scatter_order,
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

        // Collect visible scatter order and update each widget's layout rect.
        let mut new_visible_order: Vec<String> = Vec::new();
        collect_visible_scatter_ids(tree, &layout, &mut new_visible_order);
        *visible_scatter_order = new_visible_order.clone();

        for runtime in scatters.values_mut() {
            runtime.widget.set_point_size_override(None, queue);
            runtime.widget.set_point_style(None, queue);
            runtime
                .widget
                .set_layout_rect(0.0, 0.0, 0.0, 0.0, None, [0.0; 4], queue);
        }
        for scatter_id in &new_visible_order {
            if let Some(runtime) = scatters.get_mut(scatter_id) {
                let scatter_node = find_node(tree, scatter_id);
                let point_size = scatter_node
                    .and_then(|node| node.style.widget.scatter_point_size)
                    .map(|size| size * *scale_factor);
                let point_style =
                    scatter_node.and_then(|node| node.style.widget.scatter_point_style.as_deref());
                let clip_radii = scatter_node
                    .map(|node| scatter_clip_radii(node, theme.radius, *scale_factor))
                    .unwrap_or([0.0; 4]);
                runtime.widget.set_point_size_override(point_size, queue);
                runtime.widget.set_point_style(point_style, queue);
                if let (Some(r), Some(visible)) = (
                    layout.rects.get(scatter_id.as_str()).copied(),
                    layout.visible_rect(scatter_id),
                ) {
                    runtime.widget.set_layout_rect(
                        r.x,
                        r.y,
                        r.w,
                        r.h,
                        Some([visible.x, visible.y, visible.w, visible.h]),
                        clip_radii,
                        queue,
                    );
                    let (data_min, data_max) = runtime.merged_bounds();
                    if !runtime.fitted_once
                        && !runtime.points.is_empty()
                        && runtime.widget.has_visible_viewport()
                    {
                        runtime.widget.fit_to_bounds(data_min, data_max, queue);
                        runtime.fitted_once = true;
                    }
                    runtime
                        .widget
                        .refresh_grid(data_min, data_max, device, queue);
                    runtime.widget.refresh_overlays(device, queue);
                }
            }
        }

        if let Some(images) = images.as_mut() {
            images.update_screen_size(queue, config.width, config.height);
            images.rebuild(
                device,
                queue,
                tree,
                &layout,
                theme,
                *scale_factor,
                widget_state.as_ref(),
            );
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
                    media,
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
                media,
            );
        }

        *current_layout = Some(layout);
    }

    fn rebuild_primitives(&mut self) {
        let media = self.media_environment();
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
                media,
            );
        }
    }

    fn rebuild_text(&mut self) {
        let media = self.media_environment();
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
                media,
            );
        } else {
            caret_positions.clear();
        }
    }

    /// Rebuild state-dependent primitive and text buffers without recomputing layout.
    fn rebuild_visuals(&mut self) {
        self.sync_focus_transitions();
        self.sync_checked_transitions();
        self.sync_active_transitions();
        self.sync_open_transitions();
        self.sync_selected_transitions();
        self.sync_expanded_transitions();
        self.rebuild_text();
        self.rebuild_primitives();
    }

    fn current_focused_widget_ids(&self) -> HashSet<String> {
        let mut focused = HashSet::new();
        if let Some(tree) = self.widget_tree.as_ref() {
            collect_focused_widget_ids(tree, self.widget_state.as_ref(), &mut focused);
        }
        focused
    }

    fn sync_focus_transitions(&mut self) -> bool {
        let current = self.current_focused_widget_ids();
        if current == self.focus_state_snapshot {
            return false;
        }
        let old = std::mem::replace(&mut self.focus_state_snapshot, current.clone());
        let mut changed = false;
        for id in old.difference(&current) {
            changed |= self.start_focus_transition(id, 0.0);
        }
        for id in current.difference(&old) {
            changed |= self.start_focus_transition(id, 1.0);
        }
        changed
    }

    fn current_checked_widget_ids(&self) -> HashSet<String> {
        let mut checked = HashSet::new();
        if let Some(tree) = self.widget_tree.as_ref() {
            collect_checked_widget_ids(tree, self.widget_state.as_ref(), &mut checked);
        }
        checked
    }

    fn sync_checked_transitions(&mut self) -> bool {
        let current = self.current_checked_widget_ids();
        if current == self.checked_state_snapshot {
            return false;
        }
        let old = std::mem::replace(&mut self.checked_state_snapshot, current.clone());
        let mut changed = false;
        for id in old.difference(&current) {
            changed |= self.start_checked_transition(id, 0.0);
        }
        for id in current.difference(&old) {
            changed |= self.start_checked_transition(id, 1.0);
        }
        changed
    }

    fn current_active_widget_ids(&self) -> HashSet<String> {
        let mut active = HashSet::new();
        if let Some(tree) = self.widget_tree.as_ref() {
            collect_active_widget_ids(tree, self.widget_state.as_ref(), &mut active);
        }
        active
    }

    fn sync_active_transitions(&mut self) -> bool {
        let current = self.current_active_widget_ids();
        if current == self.active_state_snapshot {
            return false;
        }
        let old = std::mem::replace(&mut self.active_state_snapshot, current.clone());
        let mut changed = false;
        for id in old.difference(&current) {
            changed |= self.start_active_transition(id, 0.0);
        }
        for id in current.difference(&old) {
            changed |= self.start_active_transition(id, 1.0);
        }
        changed
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

    fn current_expanded_widget_ids(&self) -> HashSet<String> {
        let mut expanded = HashSet::new();
        if let Some(tree) = self.widget_tree.as_ref() {
            collect_expanded_widget_ids(tree, self.widget_state.as_ref(), &mut expanded);
        }
        expanded
    }

    fn sync_expanded_transitions(&mut self) -> bool {
        let current = self.current_expanded_widget_ids();
        if current == self.expanded_state_snapshot {
            return false;
        }
        let old = std::mem::replace(&mut self.expanded_state_snapshot, current.clone());
        let mut changed = false;
        for id in old.difference(&current) {
            changed |= self.start_expanded_transition(id, 0.0);
        }
        for id in current.difference(&old) {
            changed |= self.start_expanded_transition(id, 1.0);
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

    fn hover_change_requires_layout(
        &self,
        old_hover: Option<&str>,
        new_hover: Option<&str>,
    ) -> bool {
        let Some(tree) = self.widget_tree.as_ref() else {
            return false;
        };
        old_hover
            .into_iter()
            .chain(new_hover)
            .any(|id| has_rich_tooltip_for_target(tree, id))
    }

    fn current_hover_id(&self) -> Option<String> {
        self.widget_state
            .as_ref()
            .and_then(|state| state.hovered.clone())
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

    fn start_focus_transition(&mut self, id: &str, to: f32) -> bool {
        let Some((duration, delay, timing)) = self.focus_transition_config(id) else {
            self.focus_transitions.remove(id);
            if let Some(state) = &mut self.widget_state {
                state.focus_t.remove(id);
            }
            return false;
        };
        let from = self
            .widget_state
            .as_ref()
            .and_then(|state| state.focus_t.get(id).copied())
            .unwrap_or(if to >= 0.5 { 0.0 } else { 1.0 });
        if let Some(state) = &mut self.widget_state {
            state.focus_t.insert(id.to_string(), from);
        }
        self.focus_transitions.insert(
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

    fn start_checked_transition(&mut self, id: &str, to: f32) -> bool {
        let Some((duration, delay, timing)) = self.checked_transition_config(id) else {
            self.checked_transitions.remove(id);
            if let Some(state) = &mut self.widget_state {
                state.checked_t.remove(id);
            }
            return false;
        };
        let from = self
            .widget_state
            .as_ref()
            .and_then(|state| state.checked_t.get(id).copied())
            .unwrap_or(if to >= 0.5 { 0.0 } else { 1.0 });
        if let Some(state) = &mut self.widget_state {
            state.checked_t.insert(id.to_string(), from);
        }
        self.checked_transitions.insert(
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

    fn start_active_transition(&mut self, id: &str, to: f32) -> bool {
        let Some((duration, delay, timing)) = self.active_transition_config(id) else {
            self.active_transitions.remove(id);
            if let Some(state) = &mut self.widget_state {
                state.active_t.remove(id);
            }
            return false;
        };
        let from = self
            .widget_state
            .as_ref()
            .and_then(|state| state.active_t.get(id).copied())
            .unwrap_or(if to >= 0.5 { 0.0 } else { 1.0 });
        if let Some(state) = &mut self.widget_state {
            state.active_t.insert(id.to_string(), from);
        }
        self.active_transitions.insert(
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

    fn start_expanded_transition(&mut self, id: &str, to: f32) -> bool {
        let Some((duration, delay, timing)) = self.expanded_transition_config(id) else {
            self.expanded_transitions.remove(id);
            if let Some(state) = &mut self.widget_state {
                state.expanded_t.remove(id);
            }
            return false;
        };
        let from = self
            .widget_state
            .as_ref()
            .and_then(|state| state.expanded_t.get(id).copied())
            .unwrap_or(if to >= 0.5 { 0.0 } else { 1.0 });
        if let Some(state) = &mut self.widget_state {
            state.expanded_t.insert(id.to_string(), from);
        }
        self.expanded_transitions.insert(
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

    fn focus_transition_config(
        &self,
        id: &str,
    ) -> Option<(Duration, Duration, TransitionTimingFunction)> {
        let tree = self.widget_tree.as_ref()?;
        let node = find_widget(tree, id)?;
        transition_config(&node.style.transition)
    }

    fn checked_transition_config(
        &self,
        id: &str,
    ) -> Option<(Duration, Duration, TransitionTimingFunction)> {
        let tree = self.widget_tree.as_ref()?;
        let node = find_widget(tree, id)?;
        transition_config(&node.style.transition)
    }

    fn active_transition_config(
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

    fn expanded_transition_config(
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

    fn tick_focus_transitions(&mut self) -> bool {
        if self.focus_transitions.is_empty() {
            return false;
        }
        let now = Instant::now();
        let ids: Vec<String> = self.focus_transitions.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut changed = false;
        for id in ids {
            let Some(transition) = self.focus_transitions.get(&id) else {
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
                state.focus_t.insert(id.clone(), value.clamp(0.0, 1.0));
            }
            changed = true;
            if raw_t >= 1.0 {
                finished.push(id);
            }
        }
        for id in finished {
            let Some(transition) = self.focus_transitions.remove(&id) else {
                continue;
            };
            if let Some(state) = &mut self.widget_state {
                state.focus_t.remove(&id);
                if transition.to > 0.0 {
                    state.focus_t.insert(id, 1.0);
                }
            }
        }
        changed
    }

    fn tick_checked_transitions(&mut self) -> bool {
        if self.checked_transitions.is_empty() {
            return false;
        }
        let now = Instant::now();
        let ids: Vec<String> = self.checked_transitions.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut changed = false;
        for id in ids {
            let Some(transition) = self.checked_transitions.get(&id) else {
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
                state.checked_t.insert(id.clone(), value.clamp(0.0, 1.0));
            }
            changed = true;
            if raw_t >= 1.0 {
                finished.push(id);
            }
        }
        for id in finished {
            let Some(transition) = self.checked_transitions.remove(&id) else {
                continue;
            };
            if let Some(state) = &mut self.widget_state {
                state.checked_t.remove(&id);
                if transition.to > 0.0 {
                    state.checked_t.insert(id, 1.0);
                }
            }
        }
        changed
    }

    fn tick_active_transitions(&mut self) -> bool {
        if self.active_transitions.is_empty() {
            return false;
        }
        let now = Instant::now();
        let ids: Vec<String> = self.active_transitions.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut changed = false;
        for id in ids {
            let Some(transition) = self.active_transitions.get(&id) else {
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
                state.active_t.insert(id.clone(), value.clamp(0.0, 1.0));
            }
            changed = true;
            if raw_t >= 1.0 {
                finished.push(id);
            }
        }
        for id in finished {
            let Some(transition) = self.active_transitions.remove(&id) else {
                continue;
            };
            if let Some(state) = &mut self.widget_state {
                state.active_t.remove(&id);
                if transition.to > 0.0 {
                    state.active_t.insert(id, 1.0);
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

    fn tick_expanded_transitions(&mut self) -> bool {
        if self.expanded_transitions.is_empty() {
            return false;
        }
        let now = Instant::now();
        let ids: Vec<String> = self.expanded_transitions.keys().cloned().collect();
        let mut finished = Vec::new();
        let mut changed = false;
        for id in ids {
            let Some(transition) = self.expanded_transitions.get(&id) else {
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
                state.expanded_t.insert(id.clone(), value.clamp(0.0, 1.0));
            }
            changed = true;
            if raw_t >= 1.0 {
                finished.push(id);
            }
        }
        for id in finished {
            let Some(transition) = self.expanded_transitions.remove(&id) else {
                continue;
            };
            if let Some(state) = &mut self.widget_state {
                state.expanded_t.remove(&id);
                if transition.to > 0.0 {
                    state.expanded_t.insert(id, 1.0);
                }
            }
        }
        changed
    }

    fn tick_css_animations(&mut self) -> bool {
        let Some(tree) = self.widget_tree.as_ref() else {
            return false;
        };
        let keyframes = self.stylesheets.keyframes();
        if keyframes.is_empty() {
            if let Some(state) = &mut self.widget_state {
                return !std::mem::take(&mut state.animation_visuals).is_empty();
            }
            return false;
        }
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.animation_epoch);
        let mut visuals = HashMap::new();
        let mut active = false;
        collect_animation_visuals(
            tree,
            &keyframes,
            elapsed,
            &self.theme,
            &mut visuals,
            &mut active,
        );
        let Some(state) = &mut self.widget_state else {
            return false;
        };
        let had_visuals = !state.animation_visuals.is_empty();
        let has_visuals = !visuals.is_empty();
        state.animation_visuals = visuals;
        had_visuals || has_visuals || active
    }

    fn cancel_hover_transitions(&mut self) -> bool {
        clear_style_transition_state(
            &mut self.hover_transitions,
            &mut self.focus_transitions,
            &mut self.checked_transitions,
            &mut self.active_transitions,
            &mut self.open_transitions,
            &mut self.selected_transitions,
            &mut self.expanded_transitions,
            &mut self.widget_state,
        )
    }

    fn has_style_transitions(&self) -> bool {
        !self.hover_transitions.is_empty()
            || !self.focus_transitions.is_empty()
            || !self.checked_transitions.is_empty()
            || !self.active_transitions.is_empty()
            || !self.open_transitions.is_empty()
            || !self.selected_transitions.is_empty()
            || !self.expanded_transitions.is_empty()
    }

    fn has_css_animations(&self) -> bool {
        self.widget_state
            .as_ref()
            .is_some_and(|state| !state.animation_visuals.is_empty())
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

    /// Return the topmost visible scatter id that contains `pos`, or None.
    /// Checks in reverse visible order so the visually topmost scatter wins.
    fn scatter_at(&self, pos: [f32; 2]) -> Option<String> {
        for id in self.visible_scatter_order.iter().rev() {
            if let Some(runtime) = self.scatters.get(id) {
                if runtime.widget.contains_point(pos[0], pos[1]) {
                    return Some(id.clone());
                }
            }
        }
        None
    }

    fn scatter_pick_payload(&mut self, id: &str, pos: [f32; 2]) -> Option<String> {
        let runtime = self.scatters.get_mut(id)?;
        let (actor_id, index, point) = runtime.pick_all_actors_cached(pos[0], pos[1], 8.0)?;
        Some(
            json!({
                "index": index,
                "actor": actor_id,
                "x": point.position[0],
                "y": point.position[1],
                "z": point.position[2],
                "widget_id": id,
            })
            .to_string(),
        )
    }

    /// Perform polygon (lasso) selection hit test and return a JSON payload with selected indices.
    fn scatter_polygon_select_payload(&self, id: &str, poly: &[[f32; 2]]) -> Option<String> {
        let runtime = self.scatters.get(id)?;
        let mut actor_results = serde_json::Map::new();
        let indices = runtime
            .widget
            .select_points_in_polygon(&runtime.points, poly);
        actor_results.insert("0".into(), indices.into());
        for (actor_id, actor) in &runtime.widget.extra_actors {
            if actor.visible {
                let live = &actor.points[..actor.point_count as usize];
                let idx = runtime.widget.select_points_in_polygon(live, poly);
                actor_results.insert(actor_id.to_string(), idx.into());
            }
        }
        Some(
            json!({
                "event": "select",
                "widget_id": id,
                "actors": actor_results,
            })
            .to_string(),
        )
    }

    /// Perform rectangle selection hit test and return a JSON payload with selected indices.
    /// `rect` is in physical pixels relative to the scatter viewport.
    fn scatter_select_payload(&self, id: &str, rect: [f32; 4]) -> Option<String> {
        let runtime = self.scatters.get(id)?;
        let mut actor_results = serde_json::Map::new();
        // Actor 0 (primary buffer)
        let indices = runtime.widget.select_points_in_rect(&runtime.points, rect);
        actor_results.insert("0".into(), indices.into());
        // Extra actors — limit to live range to avoid picking preallocated zero-slots.
        for (actor_id, actor) in &runtime.widget.extra_actors {
            if actor.visible {
                let live = &actor.points[..actor.point_count as usize];
                let idx = runtime.widget.select_points_in_rect(live, rect);
                actor_results.insert(actor_id.to_string(), idx.into());
            }
        }
        Some(
            json!({
                "event": "select",
                "widget_id": id,
                "actors": actor_results,
            })
            .to_string(),
        )
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
                if let Some(tree) = self.widget_tree.as_mut() {
                    set_widget_checked_prop(tree, id, v);
                }
                Some(Dirty::Full)
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

    fn set_scatter_point_size_live(&mut self, id: &str, size: f32) -> bool {
        let logical_size = size.max(0.0);
        if let Some(tree) = self.widget_tree.as_mut() {
            if let Some(node) = find_widget_mut(tree, id) {
                node.style_json
                    .insert("scatter_point_size".to_string(), json!(logical_size));
                node.inline_style =
                    NodeStyle::from_json(Some(&Value::Object(node.style_json.clone())));
                node.style.widget.scatter_point_size = Some(logical_size);
            }
        }

        let Some(runtime) = self.scatters.get_mut(id) else {
            return false;
        };
        runtime
            .widget
            .set_point_size_override(Some(logical_size * self.scale_factor), &self.queue);
        runtime.primary_pick_cache = None;
        for actor in runtime.widget.extra_actors.values_mut() {
            actor.pick_cache = None;
        }
        true
    }

    fn set_scatter_point_style_live(&mut self, id: &str, style: &str) -> bool {
        if let Some(tree) = self.widget_tree.as_mut() {
            if let Some(node) = find_widget_mut(tree, id) {
                node.style_json
                    .insert("scatter_point_style".to_string(), json!(style));
                node.inline_style =
                    NodeStyle::from_json(Some(&Value::Object(node.style_json.clone())));
                node.style.widget.scatter_point_style = Some(style.to_string());
            }
        }

        let Some(runtime) = self.scatters.get_mut(id) else {
            return false;
        };
        runtime.widget.set_point_style(Some(style), &self.queue);
        true
    }

    fn add_scatter_actor_points(
        &mut self,
        id: &str,
        actor_id: u32,
        pts: Vec<PointInstance>,
        hover_meta: Option<&str>,
        tooltip_axis_labels: &[String; 3],
    ) -> bool {
        let Some(runtime) = self.scatters.get_mut(id) else {
            return false;
        };
        runtime
            .widget
            .add_actor(actor_id, pts, &self.device, &self.queue);
        if let Some(actor) = runtime.widget.extra_actors.get_mut(&actor_id) {
            actor.tooltip_axis_labels = tooltip_axis_labels.clone();
            if let Some(meta_json) = hover_meta {
                if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(meta_json) {
                    actor.hover_meta = values
                        .iter()
                        .map(|value| value.as_str().unwrap_or("").to_string())
                        .collect();
                }
            }
        }
        let (bounds_min, bounds_max) = runtime.merged_bounds();
        runtime
            .widget
            .refresh_grid(bounds_min, bounds_max, &self.device, &self.queue);
        runtime.widget.refresh_overlays(&self.device, &self.queue);
        true
    }

    fn update_scatter_actor_points(
        &mut self,
        id: &str,
        actor_id: u32,
        pts: Vec<PointInstance>,
        tooltip_axis_labels: &[String; 3],
    ) -> bool {
        let Some(runtime) = self.scatters.get_mut(id) else {
            return false;
        };
        runtime
            .widget
            .update_actor(actor_id, pts, &self.device, &self.queue);
        if let Some(actor) = runtime.widget.extra_actors.get_mut(&actor_id) {
            actor.tooltip_axis_labels = tooltip_axis_labels.clone();
            actor.pick_cache = None;
        }
        let (bounds_min, bounds_max) = runtime.merged_bounds();
        runtime
            .widget
            .refresh_grid(bounds_min, bounds_max, &self.device, &self.queue);
        runtime.widget.refresh_overlays(&self.device, &self.queue);
        true
    }

    fn stream_scatter_actor_points(
        &mut self,
        id: &str,
        actor_id: u32,
        pts: &[PointInstance],
    ) -> bool {
        let Some(runtime) = self.scatters.get_mut(id) else {
            return false;
        };
        runtime.widget.stream_actor(actor_id, pts, &self.queue);
        if let Some(actor) = runtime.widget.extra_actors.get_mut(&actor_id) {
            actor.pick_cache = None;
        }
        let (bounds_min, bounds_max) = runtime.merged_bounds();
        runtime
            .widget
            .refresh_grid(bounds_min, bounds_max, &self.device, &self.queue);
        runtime.widget.refresh_overlays(&self.device, &self.queue);
        true
    }

    fn rebuild_retained_maps(&mut self) {
        let mut widget_kinds = HashMap::new();
        let widget_state = self.widget_tree.as_ref().map(|tree| {
            collect_widget_kinds(tree, &mut widget_kinds);
            self.resources.sync_from_tree(tree);
            WidgetState::from_tree(tree)
        });
        if self.widget_tree.is_none() {
            self.resources = ResourceRegistry::default();
        }

        // Sync scatter runtime map: add new widgets, drop removed ones.
        let mut current_ids: Vec<String> = Vec::new();
        if let Some(tree) = &self.widget_tree {
            collect_all_scatter_ids(tree, &mut current_ids);
        }
        // Drop runtimes for ids no longer in the tree.
        self.scatters.retain(|id, _| current_ids.contains(id));
        // Add runtimes for newly appeared scatter nodes.
        for id in &current_ids {
            if !self.scatters.contains_key(id) {
                let node = self
                    .widget_tree
                    .as_ref()
                    .and_then(|tree| find_widget(tree, id));
                let (colormap, data_b64, data_format, startup_chrome) = node
                    .map(|n| {
                        let chrome = scatter_chrome_from_props(&n.props);
                        (
                            n.props
                                .scatter_colormap
                                .clone()
                                .unwrap_or_else(|| "viridis".to_string()),
                            n.props.scatter_data_b64.clone(),
                            n.props.scatter_data_format,
                            chrome,
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            "viridis".to_string(),
                            None,
                            ScatterPayloadFormat::default(),
                            scatter::ScatterChromeState::default(),
                        )
                    });

                let mut widget = ScatterWidget::new(
                    &self.device,
                    self.config.format,
                    self.config.width.max(1),
                    self.config.height.max(1),
                );
                let (mut pts, mut status) = if let Some(b64) = data_b64 {
                    let decoded_bytes = BASE64.decode(&b64);
                    match decoded_bytes {
                        Err(e) => {
                            let msg = format!("scatter base64 decode: {e}");
                            eprintln!("DragonGUI: {msg}");
                            (Vec::new(), ScatterPayloadStatus::DecodeError(msg))
                        }
                        Ok(bytes) => {
                            let mut pts = Vec::new();
                            let result = match data_format {
                                ScatterPayloadFormat::XyzF32V0 => {
                                    decode_scatter_points_bytes_into_colormap(
                                        &bytes, &mut pts, &colormap,
                                    )
                                }
                                ScatterPayloadFormat::PointInstanceV1 => {
                                    decode_scatter_points_v1(&bytes, &mut pts)
                                }
                            };
                            match result {
                                Err(e) => {
                                    let msg = e.to_string();
                                    eprintln!("DragonGUI: {msg}");
                                    (Vec::new(), ScatterPayloadStatus::DecodeError(msg))
                                }
                                Ok(_) => (pts, ScatterPayloadStatus::Ok),
                            }
                        }
                    }
                } else {
                    (Vec::new(), ScatterPayloadStatus::Empty)
                };

                let maybe_bounds = compute_scatter_bounds(&pts);
                if maybe_bounds.is_none() && !pts.is_empty() {
                    status = ScatterPayloadStatus::AllNonFinite;
                    pts.clear();
                }
                let (data_min, data_max) =
                    maybe_bounds.unwrap_or((glam::Vec3::ZERO, glam::Vec3::ZERO));
                if !pts.is_empty() {
                    widget.set_points(&self.device, &self.queue, &pts);
                    widget.fit_to_bounds(data_min, data_max, &self.queue);
                } else {
                    widget.update_camera(&self.queue);
                }
                widget.set_chrome(startup_chrome);
                widget.refresh_grid(data_min, data_max, &self.device, &self.queue);
                widget.refresh_overlays(&self.device, &self.queue);

                self.scatters.insert(
                    id.clone(),
                    ScatterRuntime {
                        widget,
                        points: pts,
                        metrics: ScatterMetrics::default(),
                        fitted_once: matches!(status, ScatterPayloadStatus::Ok),
                        data_min,
                        data_max,
                        payload_format: data_format,
                        payload_status: status,
                        primary_hover_meta: Vec::new(),
                        tooltip_axis_labels: ["x".to_string(), "y".to_string(), "z".to_string()],
                        hover_tooltip_enabled: true,
                        primary_pick_cache: None,
                    },
                );
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
        data_format: ScatterPayloadFormat,
    ) -> Result<bool, DragonError> {
        if self.widget_kind(id) != Some(WidgetKind::Scatter3D) {
            return Ok(false);
        }
        let Some(runtime) = self.scatters.get_mut(id) else {
            return Ok(false);
        };
        let total_t0 = Instant::now();
        let queue_latency_ms = telemetry
            .as_ref()
            .map(|t| (now_epoch_ms() - t.enqueue_epoch_ms).max(0.0))
            .unwrap_or(0.0);

        let decode_t0 = Instant::now();
        let mut decoded = std::mem::take(&mut runtime.points);
        let result = match data_format {
            ScatterPayloadFormat::XyzF32V0 => {
                decode_scatter_points_bytes_into_colormap(&xyz, &mut decoded, &colormap)
            }
            ScatterPayloadFormat::PointInstanceV1 => decode_scatter_points_v1(&xyz, &mut decoded),
        };
        let maybe_bounds = match result {
            Ok(bounds) => bounds,
            Err(e) => {
                runtime.points = decoded;
                let msg = e.to_string();
                eprintln!("DragonGUI: {msg}");
                runtime.payload_status = ScatterPayloadStatus::DecodeError(msg.clone());
                // Do not mutate runtime.points or payload_format — preserve last valid state.
                return Err(DragonError::Runtime(msg));
            }
        };
        let decode_ms = decode_t0.elapsed().as_secs_f64() * 1000.0;
        let bounds_ms = 0.0;

        if maybe_bounds.is_none() && !decoded.is_empty() {
            runtime.payload_format = data_format;
            decoded.clear();
            runtime.points = decoded;
            runtime.primary_pick_cache = None;
            runtime.payload_status = ScatterPayloadStatus::AllNonFinite;
            runtime.primary_hover_meta = Vec::new();
            runtime.widget.hover_label = None;
            runtime.data_min = glam::Vec3::ZERO;
            runtime.data_max = glam::Vec3::ZERO;
            // Clear the GPU draw count so the old geometry is not rendered.
            let upload_timings = runtime.widget.set_points(&self.device, &self.queue, &[]);
            let grid_t0 = Instant::now();
            runtime.widget.refresh_grid(
                glam::Vec3::ZERO,
                glam::Vec3::ZERO,
                &self.device,
                &self.queue,
            );
            let grid_ms = grid_t0.elapsed().as_secs_f64() * 1000.0;
            let overlay_t0 = Instant::now();
            runtime.widget.refresh_overlays(&self.device, &self.queue);
            let overlay_ms = overlay_t0.elapsed().as_secs_f64() * 1000.0;
            let pack_ms = telemetry.as_ref().map(|t| t.pack_ms).unwrap_or(0.0);
            let reported_payload_bytes = telemetry
                .as_ref()
                .map(|t| t.payload_bytes)
                .unwrap_or(xyz.len());
            runtime.metrics = ScatterMetrics {
                updates: runtime.metrics.updates + 1,
                last_point_count: 0,
                last_payload_bytes: reported_payload_bytes,
                last_pack_ms: pack_ms,
                last_queue_latency_ms: queue_latency_ms,
                last_decode_ms: decode_ms,
                last_bounds_ms: bounds_ms,
                last_upload_ms: upload_timings.primary_ms
                    + upload_timings.lod_ms
                    + grid_ms
                    + overlay_ms,
                last_primary_upload_ms: upload_timings.primary_ms,
                last_lod_ms: upload_timings.lod_ms,
                last_grid_ms: grid_ms,
                last_overlay_ms: overlay_ms,
                last_total_native_ms: total_t0.elapsed().as_secs_f64() * 1000.0,
            };
            return Ok(true);
        }
        let (data_min, data_max) = maybe_bounds.unwrap_or((glam::Vec3::ZERO, glam::Vec3::ZERO));
        runtime.payload_format = data_format;
        runtime.points = decoded;
        runtime.primary_pick_cache = None;
        runtime.primary_hover_meta = Vec::new();
        runtime.widget.hover_label = None;
        runtime.data_min = data_min;
        runtime.data_max = data_max;
        runtime.payload_status = ScatterPayloadStatus::Ok;

        let upload_t0 = Instant::now();
        let upload_timings = runtime
            .widget
            .set_points(&self.device, &self.queue, &runtime.points);
        // Fit on first visible data load; preserve camera for subsequent live updates.
        //
        // Startup resource uploads can arrive while a scatter is hidden inside
        // an inactive page. Hidden scatters have a zero-sized layout rect, and
        // fitting against that aspect ratio pushes the camera thousands of
        // units away. Leave `fitted_once` false so the next visible layout pass
        // can fit with the real viewport dimensions.
        if !runtime.fitted_once
            && !runtime.points.is_empty()
            && runtime.widget.has_visible_viewport()
        {
            runtime
                .widget
                .fit_to_bounds(data_min, data_max, &self.queue);
            runtime.fitted_once = true;
        }
        let grid_t0 = Instant::now();
        runtime
            .widget
            .refresh_grid(data_min, data_max, &self.device, &self.queue);
        let grid_ms = grid_t0.elapsed().as_secs_f64() * 1000.0;
        let overlay_t0 = Instant::now();
        runtime.widget.refresh_overlays(&self.device, &self.queue);
        let overlay_ms = overlay_t0.elapsed().as_secs_f64() * 1000.0;
        let upload_ms = upload_t0.elapsed().as_secs_f64() * 1000.0;

        let point_count = runtime.points.len();
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
        runtime.metrics = ScatterMetrics {
            updates: runtime.metrics.updates + 1,
            last_point_count: reported_point_count,
            last_payload_bytes: reported_payload_bytes,
            last_pack_ms: pack_ms,
            last_queue_latency_ms: queue_latency_ms,
            last_decode_ms: decode_ms,
            last_bounds_ms: bounds_ms,
            last_upload_ms: upload_ms,
            last_primary_upload_ms: upload_timings.primary_ms,
            last_lod_ms: upload_timings.lod_ms,
            last_grid_ms: grid_ms,
            last_overlay_ms: overlay_ms,
            last_total_native_ms: total_t0.elapsed().as_secs_f64() * 1000.0,
        };
        Ok(true)
    }

    fn sync_scatter_style_overrides(&mut self) {
        let Some(tree) = self.widget_tree.as_ref() else {
            return;
        };
        let overrides: Vec<(String, Option<f32>, Option<String>)> = self
            .visible_scatter_order
            .iter()
            .map(|id| {
                let node = find_node(tree, id);
                (
                    id.clone(),
                    node.and_then(|node| node.style.widget.scatter_point_size)
                        .map(|size| size * self.scale_factor),
                    node.and_then(|node| node.style.widget.scatter_point_style.clone()),
                )
            })
            .collect();
        for (id, point_size, point_style) in overrides {
            let Some(runtime) = self.scatters.get_mut(&id) else {
                continue;
            };
            let old_point_size = runtime.widget.point_size_override;
            runtime
                .widget
                .set_point_size_override(point_size, &self.queue);
            if runtime.widget.point_size_override.to_bits() != old_point_size.to_bits() {
                runtime.primary_pick_cache = None;
                for actor in runtime.widget.extra_actors.values_mut() {
                    actor.pick_cache = None;
                }
            }
            runtime
                .widget
                .set_point_style(point_style.as_deref(), &self.queue);
        }
    }

    fn rebuild_for_dirty(&mut self, dirty: Dirty) {
        if matches!(dirty, Dirty::Layout | Dirty::Full) {
            self.cancel_hover_transitions();
        }
        match dirty {
            Dirty::Layout | Dirty::Full => self.apply_layout(),
            Dirty::Text => self.rebuild_visuals(),
            Dirty::Visual => {
                self.sync_scatter_style_overrides();
                self.rebuild_primitives();
            }
            Dirty::GpuData => {}
        }
    }

    fn reapply_stylesheets(&mut self) {
        self.cancel_hover_transitions();
        self.mark_styles_dirty();
        self.reapply_stylesheets_for_current_viewport();
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
            "computed_styles": computed_styles_snapshot(
                self.widget_tree.as_ref(),
                &self.stylesheets,
                Some(self.media_environment()),
            ),
            "tree": self.widget_tree.as_ref().map(node_snapshot),
            "layout": layout_snapshot(self.current_layout.as_ref()),
            "state": widget_state_snapshot(self.widget_state.as_ref()),
            "toasts": self.toast_snapshot(),
            "renderer": {
                "surface_format": format!("{:?}", self.config.format),
                "has_primitives": self.primitives.is_some(),
                "has_text": self.text.is_some(),
                "font_warnings": self.text.as_ref().map(|text| text.font_warnings()).unwrap_or(&[]),
                "has_scatter": !self.scatters.is_empty(),
                "scatter_widget_id": self.visible_scatter_order.first().map(|s| s.as_str()),
                "scatter_count": self.scatters.len(),
                "widget_count": self.widget_kinds.len(),
                "caret_positions": &self.caret_positions,
            },
            "resources": {
                "scatter": self.visible_scatter_order.first()
                    .and_then(|id| self.scatters.get(id).map(|rt| (id, rt)))
                    .map(|(id, rt)| json!({
                    "id": id,
                    "updates": rt.metrics.updates,
                    "last_point_count": rt.metrics.last_point_count,
                    "last_payload_bytes": rt.metrics.last_payload_bytes,
                    "last_pack_ms": rt.metrics.last_pack_ms,
                    "last_queue_latency_ms": rt.metrics.last_queue_latency_ms,
                    "last_decode_ms": rt.metrics.last_decode_ms,
                    "last_bounds_ms": rt.metrics.last_bounds_ms,
                    "last_upload_ms": rt.metrics.last_upload_ms,
                    "last_primary_upload_ms": rt.metrics.last_primary_upload_ms,
                    "last_lod_ms": rt.metrics.last_lod_ms,
                    "last_grid_ms": rt.metrics.last_grid_ms,
                    "last_overlay_ms": rt.metrics.last_overlay_ms,
                    "last_total_native_ms": rt.metrics.last_total_native_ms,
                    "payload_status": format!("{:?}", rt.payload_status),
                })),
                "scatters": self.scatters.iter().map(|(id, rt)| {
                    let cs = rt.widget.camera_state();
                    (id.clone(), json!({
                        "updates": rt.metrics.updates,
                        "last_point_count": rt.metrics.last_point_count,
                        "last_payload_bytes": rt.metrics.last_payload_bytes,
                        "last_pack_ms": rt.metrics.last_pack_ms,
                        "last_queue_latency_ms": rt.metrics.last_queue_latency_ms,
                        "last_decode_ms": rt.metrics.last_decode_ms,
                        "last_bounds_ms": rt.metrics.last_bounds_ms,
                        "last_upload_ms": rt.metrics.last_upload_ms,
                        "last_primary_upload_ms": rt.metrics.last_primary_upload_ms,
                        "last_lod_ms": rt.metrics.last_lod_ms,
                        "last_grid_ms": rt.metrics.last_grid_ms,
                        "last_overlay_ms": rt.metrics.last_overlay_ms,
                        "last_total_native_ms": rt.metrics.last_total_native_ms,
                        "payload_status": format!("{:?}", rt.payload_status),
                        "fitted_once": rt.fitted_once,
                        "payload_format": rt.payload_format.as_str(),
                        "camera": {
                            "target": cs.target,
                            "distance": cs.distance,
                            "yaw": cs.yaw,
                            "pitch": cs.pitch,
                            "parallel": cs.parallel,
                        },
                        "lod": {
                            "enabled": rt.widget.lod_enabled,
                            "active": rt.widget.lod_active,
                            "threshold": rt.widget.lod_threshold,
                            "factor": rt.widget.lod_factor,
                        },
                    }))
                }).collect::<serde_json::Map<_, _>>(),
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

    fn has_open_popup(&self) -> bool {
        self.widget_state.as_ref().is_some_and(|state| {
            state.open_dropdown.is_some()
                || state.open_menu.is_some()
                || state.open_context_menu.is_some()
        })
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

    fn keyboard_scroll_container_target(
        &self,
        fallback_pos: Option<[f32; 2]>,
    ) -> Option<ScrollContainerKeyboardTarget> {
        let tree = self.widget_tree.as_ref()?;
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        if let Some(target) = state.focused.as_deref().and_then(|focused| {
            focused_scroll_container_keyboard_target(tree, layout, state, focused, None)
        }) {
            return Some(target);
        }
        let id = fallback_pos.and_then(|pos| scroll_container_at_pos(tree, layout, state, pos))?;
        let node = find_widget(tree, &id)?;
        scroll_container_keyboard_target_by_id(node, layout, state)
    }

    fn panel_scrollbar_at(&self, pos: [f32; 2]) -> Option<PanelScrollbarHit> {
        let tree = self.widget_tree.as_ref()?;
        let layout = self.current_layout.as_ref()?;
        let state = self.widget_state.as_ref()?;
        let root = active_modal_ref(tree).unwrap_or(tree);
        let slop = (5.0 * self.scale_factor).max(4.0);
        self.panel_scrollbar_at_node(root, layout, state, pos, slop)
    }

    fn panel_scrollbar_at_node(
        &self,
        node: &WidgetNode,
        layout: &crate::layout::LayoutResult,
        state: &WidgetState,
        pos: [f32; 2],
        slop: f32,
    ) -> Option<PanelScrollbarHit> {
        if is_scroll_container_node(node) && !state.is_disabled(&node.id) {
            if let (Some(visible), Some(rect)) = (
                layout.visible_rect(&node.id),
                layout.rects.get(&node.id).copied(),
            ) {
                if let Some(geometry) = panel_scrollbar_geometry(
                    node,
                    layout,
                    state,
                    &self.theme,
                    self.scale_factor,
                    rect,
                ) {
                    let pos_inside_visible = pos[0] >= visible.x
                        && pos[0] < visible.x + visible.w
                        && pos[1] >= visible.y
                        && pos[1] < visible.y + visible.h;
                    if !pos_inside_visible {
                        return None;
                    }
                    if let Some(hit) = panel_scrollbar_axis_hit(
                        &node.id,
                        PanelScrollbarAxis::Horizontal,
                        geometry.horizontal,
                        pos,
                        slop,
                    ) {
                        return Some(hit);
                    }
                    if let Some(hit) = panel_scrollbar_axis_hit(
                        &node.id,
                        PanelScrollbarAxis::Vertical,
                        geometry.vertical,
                        pos,
                        slop,
                    ) {
                        return Some(hit);
                    }
                }
            }
        }
        for child in node.children.iter().rev() {
            if let Some(hit) = self.panel_scrollbar_at_node(child, layout, state, pos, slop) {
                return Some(hit);
            }
        }
        None
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

    fn scroll_container(&mut self, id: &str, wheel_x: f32, wheel_y: f32) -> bool {
        let Some(tree) = self.widget_tree.as_ref() else {
            return false;
        };
        let Some(layout) = self.current_layout.as_ref() else {
            return false;
        };
        let Some(node) = find_widget(tree, id) else {
            return false;
        };
        let max_scroll_x = layout
            .scroll_max_x
            .get(id)
            .copied()
            .unwrap_or_else(|| scroll_container_max_x(node, layout));
        let max_scroll_y = layout
            .scroll_max_y
            .get(id)
            .copied()
            .unwrap_or_else(|| scroll_container_max_y(node, layout));
        if max_scroll_x <= 0.0 && max_scroll_y <= 0.0 {
            return false;
        }
        let line = self.theme.control_height() * self.scale_factor * 0.75;
        let delta_x = -wheel_x * line;
        let delta_y = -wheel_y * line;
        let changed = self
            .widget_state
            .as_mut()
            .map(|state| state.scroll_container(id, delta_x, delta_y, max_scroll_x, max_scroll_y))
            .unwrap_or(false);
        if changed {
            self.apply_layout();
        }
        changed
    }

    fn scroll_container_to_axis(
        &mut self,
        id: &str,
        axis: PanelScrollbarAxis,
        scroll: f32,
    ) -> bool {
        let Some(tree) = self.widget_tree.as_ref() else {
            return false;
        };
        let Some(layout) = self.current_layout.as_ref() else {
            return false;
        };
        let Some(node) = find_widget(tree, id) else {
            return false;
        };
        let max_scroll_x = layout
            .scroll_max_x
            .get(id)
            .copied()
            .unwrap_or_else(|| scroll_container_max_x(node, layout));
        let max_scroll_y = layout
            .scroll_max_y
            .get(id)
            .copied()
            .unwrap_or_else(|| scroll_container_max_y(node, layout));
        let Some(state) = self.widget_state.as_mut() else {
            return false;
        };
        let current_x = state.container_scroll_x(id, max_scroll_x);
        let current_y = state.container_scroll_y(id, max_scroll_y);
        let (delta_x, delta_y) = match axis {
            PanelScrollbarAxis::Horizontal => (scroll - current_x, 0.0),
            PanelScrollbarAxis::Vertical => (0.0, scroll - current_y),
        };
        let changed = state.scroll_container(id, delta_x, delta_y, max_scroll_x, max_scroll_y);
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
                scatters,
                visible_scatter_order,
                scale_factor,
                device,
                queue,
                ..
            } = &mut *self;
            if let Some(t) = text.as_mut() {
                // Rebuild scatter grid labels from each scatter's pending_labels.
                t.clear_scatter_labels();
                for id in visible_scatter_order.iter() {
                    if let Some(rt) = scatters.get(id) {
                        let [vl, vt, vr, vb] = rt.widget.viewport_clip();
                        let clip = glyphon::TextBounds {
                            left: vl as i32,
                            top: vt as i32,
                            right: vr as i32,
                            bottom: vb as i32,
                        };
                        for lbl in &rt.widget.pending_labels {
                            t.push_scatter_label(
                                &lbl.text,
                                lbl.screen_x,
                                lbl.screen_y,
                                lbl.is_title,
                                clip,
                                *scale_factor,
                                lbl.color,
                                lbl.font_size,
                                &lbl.anchor,
                            );
                        }
                    }
                }
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

        // Pass 1: base primitives and images — clear color and depth.
        {
            let bg = self.theme.background;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dragongui-base"),
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
            if let Some(prims) = &self.primitives {
                prims.render_base(&mut pass);
            }
            if let Some(images) = &self.images {
                images.render(&mut pass);
            }
        }

        // Pass 2: one render pass per visible scatter with its own depth clear
        // to prevent cross-widget depth contamination.
        let scatter_order = self.visible_scatter_order.clone();
        for scatter_id in &scatter_order {
            if let Some(runtime) = self.scatters.get(scatter_id) {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("dragongui-scatter"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
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
                runtime.widget.render(&mut pass);
            }
        }

        // Pass 3: text and overlay primitives. The overlay pipelines do not
        // write depth and always pass, but wgpu still requires a depth
        // attachment because those pipelines were created with Depth32Float.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dragongui-overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                0.0,
                0.0,
                self.config.width as f32,
                self.config.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
            if let Some(t) = &self.text {
                t.render_base(&mut pass);
            }
            if let Some(prims) = &self.primitives {
                prims.render_overlays(&mut pass);
            }
            if let Some(t) = &self.text {
                t.render_overlays(&mut pass);
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

fn scrollbar_rect_contains(
    axis: PanelScrollbarAxis,
    r: &crate::layout::Rect,
    pos: [f32; 2],
    slop: f32,
) -> bool {
    match axis {
        PanelScrollbarAxis::Horizontal => {
            pos[0] >= r.x && pos[0] < r.x + r.w && pos[1] >= r.y - slop && pos[1] < r.y + r.h + slop
        }
        PanelScrollbarAxis::Vertical => {
            pos[0] >= r.x - slop && pos[0] < r.x + r.w + slop && pos[1] >= r.y && pos[1] < r.y + r.h
        }
    }
}

fn panel_scrollbar_axis_hit(
    widget_id: &str,
    axis: PanelScrollbarAxis,
    geometry: Option<PanelScrollbarAxisGeometry>,
    pos: [f32; 2],
    slop: f32,
) -> Option<PanelScrollbarHit> {
    let geometry = geometry?;
    let on_thumb = scrollbar_rect_contains(axis, &geometry.thumb, pos, slop);
    if on_thumb || scrollbar_rect_contains(axis, &geometry.track, pos, slop) {
        Some(PanelScrollbarHit {
            widget_id: widget_id.to_string(),
            axis,
            geometry,
            on_thumb,
        })
    } else {
        None
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
struct PanelScrollbarHit {
    widget_id: String,
    axis: PanelScrollbarAxis,
    geometry: PanelScrollbarAxisGeometry,
    on_thumb: bool,
}

#[derive(Debug, Clone)]
struct ScrollbarDrag {
    widget_id: String,
    axis: PanelScrollbarAxis,
    track_start: f32,
    track_len: f32,
    thumb_len: f32,
    grab_offset: f32,
    max_scroll: f32,
}

impl ScrollbarDrag {
    fn new(hit: PanelScrollbarHit, pos: [f32; 2]) -> Self {
        let axis_pos = scrollbar_axis_pos(hit.axis, pos);
        let thumb_start = scrollbar_axis_start(hit.axis, hit.geometry.thumb);
        let grab_offset = if hit.on_thumb {
            (axis_pos - thumb_start).clamp(0.0, scrollbar_axis_len(hit.axis, hit.geometry.thumb))
        } else {
            scrollbar_axis_len(hit.axis, hit.geometry.thumb) * 0.5
        };
        Self {
            widget_id: hit.widget_id,
            axis: hit.axis,
            track_start: scrollbar_axis_start(hit.axis, hit.geometry.track),
            track_len: scrollbar_axis_len(hit.axis, hit.geometry.track),
            thumb_len: scrollbar_axis_len(hit.axis, hit.geometry.thumb),
            grab_offset,
            max_scroll: hit.geometry.max_scroll,
        }
    }

    fn compute_scroll(&self, pos: [f32; 2]) -> f32 {
        let travel = (self.track_len - self.thumb_len).max(0.0);
        if travel <= f32::EPSILON {
            return 0.0;
        }
        let thumb_start = scrollbar_axis_pos(self.axis, pos) - self.grab_offset;
        let t = ((thumb_start - self.track_start) / travel).clamp(0.0, 1.0);
        t * self.max_scroll.max(0.0)
    }
}

fn scrollbar_axis_pos(axis: PanelScrollbarAxis, pos: [f32; 2]) -> f32 {
    match axis {
        PanelScrollbarAxis::Horizontal => pos[0],
        PanelScrollbarAxis::Vertical => pos[1],
    }
}

fn scrollbar_axis_start(axis: PanelScrollbarAxis, rect: crate::layout::Rect) -> f32 {
    match axis {
        PanelScrollbarAxis::Horizontal => rect.x,
        PanelScrollbarAxis::Vertical => rect.y,
    }
}

fn scrollbar_axis_len(axis: PanelScrollbarAxis, rect: crate::layout::Rect) -> f32 {
    match axis {
        PanelScrollbarAxis::Horizontal => rect.w,
        PanelScrollbarAxis::Vertical => rect.h,
    }
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
    /// True while the user is dragging a rectangle selection (rectangle picking mode).
    rect_select_active: bool,
    /// Button on_click callbacks (moved out of AppSpec in `resumed`).
    click_cbs: HashMap<String, Box<dyn Fn() + Send>>,
    /// Checkbox / Slider on_change callbacks.
    change_cbs: HashMap<String, Box<dyn Fn(ChangeValue) + Send>>,
    /// Active slider drag session (pointer-down on a Slider widget).
    slider_drag: Option<SliderDrag>,
    /// Active panel scrollbar drag session.
    scrollbar_drag: Option<ScrollbarDrag>,
    scatter_press_pos: Option<[f32; 2]>,
    /// Scatter id that received the current pointer-down (for orbit/pan/pick).
    active_scatter_id: Option<String>,
    /// Last slider value sent to Python during drag throttling.
    last_slider_emit: Option<SliderChangeDispatch>,
    /// Most recent slider value waiting for a throttled callback slot.
    pending_slider_emit: Option<(String, f32)>,
    /// Id of the UI widget that received the current pointer-down.
    pressed_id: Option<String>,
    /// Currently active keyboard modifiers.
    modifiers: ModifiersState,
    /// Whether a background Python task drain was held while a transient popup was open.
    deferred_python_task_drain: bool,
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
            rect_select_active: false,
            click_cbs: HashMap::new(),
            change_cbs: HashMap::new(),
            slider_drag: None,
            scrollbar_drag: None,
            scatter_press_pos: None,
            active_scatter_id: None,
            last_slider_emit: None,
            pending_slider_emit: None,
            pressed_id: None,
            modifiers: ModifiersState::empty(),
            deferred_python_task_drain: false,
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
                "scrollbar_drag": self.scrollbar_drag.as_ref().map(|drag| json!({
                    "id": drag.widget_id,
                    "axis": match drag.axis {
                        PanelScrollbarAxis::Horizontal => "horizontal",
                        PanelScrollbarAxis::Vertical => "vertical",
                    },
                })),
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

    fn has_open_popup(&self) -> bool {
        self.gpu.as_ref().is_some_and(WgpuState::has_open_popup)
    }

    fn defer_runtime_command_while_popup_open(
        &mut self,
        command: Command,
    ) -> Result<bool, Command> {
        if !self.has_open_popup() {
            return Err(command);
        }
        match command {
            Command::DrainPythonTasks => {
                self.deferred_python_task_drain = true;
                Ok(self.record_runtime_command(
                    "DrainPythonTasks",
                    None,
                    Some("deferred while popup open".to_string()),
                    None,
                    "deferred_interactive_popup",
                    false,
                ))
            }
            command => Err(command),
        }
    }

    fn flush_deferred_popup_commands(&mut self) -> bool {
        if self.has_open_popup() {
            return false;
        }
        let mut request_redraw = false;
        if self.deferred_python_task_drain {
            self.deferred_python_task_drain = false;
            request_redraw |= self.apply_runtime_command(Command::DrainPythonTasks);
        }
        request_redraw
    }

    fn drain_runtime_commands(&mut self) {
        if self.window.is_none() || self.gpu.is_none() {
            return;
        }
        let Some(bridge) = self.command_bridge.as_ref().cloned() else {
            return;
        };
        bridge.clear_wake_pending();

        let mut request_redraw = false;
        let mut commands = Vec::new();
        let mut batches = 0_usize;
        let drain_start = Instant::now();
        loop {
            commands.clear();
            bridge.drain_limited_into(&mut commands, MAX_COMMANDS_PER_DRAIN_BATCH);
            coalesce_runtime_command_batch(&mut commands);
            if commands.is_empty() {
                break;
            }
            batches += 1;
            for command in commands.drain(..) {
                request_redraw |= self.apply_runtime_command(command);
            }
            if batches >= MAX_COMMAND_DRAIN_BATCHES || drain_start.elapsed() >= COMMAND_DRAIN_BUDGET
            {
                let pending = bridge.len();
                if pending > 512 {
                    eprintln!(
                        "DragonGUI: command drain reached fairness limit; deferring {pending} pending commands"
                    );
                }
                if pending > 0 {
                    bridge.wake();
                }
                break;
            }
        }

        request_redraw |= self.flush_deferred_popup_commands();

        if request_redraw {
            self.request_redraw();
        }
    }

    fn apply_runtime_command(&mut self, command: Command) -> bool {
        let command = match self.defer_runtime_command_while_popup_open(command) {
            Ok(deferred) => return deferred,
            Err(command) => command,
        };
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
                payload_format,
                coalesce: _,
            } => {
                let detail = Some(format!(
                    "payload_bytes={}, colormap={colormap}, format={}",
                    xyz.len(),
                    payload_format.as_str()
                ));
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
                    match gpu.set_scatter_points_packed(
                        &id,
                        xyz,
                        telemetry,
                        colormap,
                        payload_format,
                    ) {
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
            Command::SetScatterPrimaryHoverMeta { id, meta } => {
                let ok = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&meta) {
                        rt.primary_hover_meta = values
                            .iter()
                            .map(|v| v.as_str().unwrap_or("").to_string())
                            .collect();
                    }
                    true
                });
                self.record_runtime_command(
                    "SetScatterPrimaryHoverMeta",
                    Some(id),
                    None,
                    None,
                    if ok { "ok" } else { "no-op: scatter not found" },
                    false,
                )
            }
            Command::SetScatterTooltipAxisLabels { id, labels } => {
                let ok = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.tooltip_axis_labels = labels;
                    true
                });
                self.record_runtime_command(
                    "SetScatterTooltipAxisLabels",
                    Some(id),
                    None,
                    None,
                    if ok { "ok" } else { "no-op: scatter not found" },
                    false,
                )
            }
            Command::ResetScatterCamera { id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    if let Some(rt) = gpu.scatters.get_mut(&id) {
                        rt.widget.reset_camera(&gpu.queue);
                        let (bmn, bmx) = rt.merged_bounds();
                        rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                        rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                        true
                    } else {
                        false
                    }
                });
                self.record_runtime_command(
                    "ResetScatterCamera",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterViewDirection { id, direction } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    if let Some(rt) = gpu.scatters.get_mut(&id) {
                        rt.widget.set_view_direction(&direction, &gpu.queue);
                        let (bmn, bmx) = rt.merged_bounds();
                        rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                        rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                        true
                    } else {
                        false
                    }
                });
                self.record_runtime_command(
                    "SetScatterViewDirection",
                    Some(id),
                    Some(direction),
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::FitScatterCamera { id, bounds } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    if let Some(b) = bounds {
                        let min = glam::Vec3::new(b[0], b[1], b[2]);
                        let max = glam::Vec3::new(b[3], b[4], b[5]);
                        // Do not overwrite data_min/data_max — fit is a camera operation only.
                        rt.widget.fit_to_bounds(min, max, &gpu.queue);
                        rt.fitted_once = true;
                    } else if !rt.points.is_empty() || rt.widget.merged_extra_bounds().is_some() {
                        let (bmn, bmx) = rt.merged_bounds();
                        rt.widget.fit_to_bounds(bmn, bmx, &gpu.queue);
                    }
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "FitScatterCamera",
                    Some(id),
                    bounds.map(|b| format!("{b:?}")),
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterParallelProjection { id, parallel } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    if let Some(rt) = gpu.scatters.get_mut(&id) {
                        rt.widget.set_parallel_projection(parallel, &gpu.queue);
                        let (bmn, bmx) = rt.merged_bounds();
                        rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                        rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                        true
                    } else {
                        false
                    }
                });
                self.record_runtime_command(
                    "SetScatterParallelProjection",
                    Some(id),
                    Some(parallel.to_string()),
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterCameraState {
                id,
                target,
                distance,
                yaw,
                pitch,
                parallel,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    if let Some(rt) = gpu.scatters.get_mut(&id) {
                        let state = crate::scatter::camera::CameraState {
                            target,
                            distance,
                            yaw,
                            pitch,
                            parallel,
                        };
                        rt.widget.set_camera_state(state, &gpu.queue);
                        let (bmn, bmx) = rt.merged_bounds();
                        rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                        rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                        true
                    } else {
                        false
                    }
                });
                self.record_runtime_command(
                    "SetScatterCameraState",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterPointStyle { id, style } => {
                let redraw = self
                    .gpu
                    .as_mut()
                    .is_some_and(|gpu| gpu.set_scatter_point_style_live(&id, &style));
                self.record_runtime_command(
                    "SetScatterPointStyle",
                    Some(id),
                    Some(style),
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterPointSize { id, size } => {
                let redraw = self
                    .gpu
                    .as_mut()
                    .is_some_and(|gpu| gpu.set_scatter_point_size_live(&id, size));
                self.record_runtime_command(
                    "SetScatterPointSize",
                    Some(id),
                    Some(format!("{size:.3}")),
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterGridVisible { id, visible } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.grid_visible = visible;
                    rt.widget.chrome_dirty = true;
                    rt.widget.refresh_grid(
                        rt.merged_bounds().0,
                        rt.merged_bounds().1,
                        &gpu.device,
                        &gpu.queue,
                    );
                    true
                });
                self.record_runtime_command(
                    "SetScatterGridVisible",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterGridPlanes { id, major, minor } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.major_planes = major;
                    rt.widget.chrome.minor_planes = minor;
                    rt.widget.chrome_dirty = true;
                    rt.widget.refresh_grid(
                        rt.merged_bounds().0,
                        rt.merged_bounds().1,
                        &gpu.device,
                        &gpu.queue,
                    );
                    true
                });
                self.record_runtime_command(
                    "SetScatterGridPlanes",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterGridOptions {
                id,
                sticky,
                all_edges,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.grid_sticky = sticky;
                    rt.widget.chrome.grid_all_edges = all_edges;
                    if !sticky {
                        rt.widget.grid_display_bounds = None;
                    }
                    rt.widget.chrome_dirty = true;
                    rt.widget.refresh_grid(
                        rt.merged_bounds().0,
                        rt.merged_bounds().1,
                        &gpu.device,
                        &gpu.queue,
                    );
                    true
                });
                self.record_runtime_command(
                    "SetScatterGridOptions",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterTicks { id, x, y, z } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.tick_override = [x, y, z];
                    rt.widget.chrome_dirty = true;
                    rt.widget.refresh_grid(
                        rt.merged_bounds().0,
                        rt.merged_bounds().1,
                        &gpu.device,
                        &gpu.queue,
                    );
                    true
                });
                self.record_runtime_command(
                    "SetScatterTicks",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterAxes { id, x, y, z } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.axis_labels = [x, y, z];
                    rt.widget.chrome_dirty = true;
                    rt.widget.refresh_grid(
                        rt.merged_bounds().0,
                        rt.merged_bounds().1,
                        &gpu.device,
                        &gpu.queue,
                    );
                    true
                });
                self.record_runtime_command(
                    "SetScatterAxes",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterAxisVisibility { id, x, y, z } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.axis_visible = [x, y, z];
                    rt.widget.chrome_dirty = true;
                    rt.widget.refresh_grid(
                        rt.merged_bounds().0,
                        rt.merged_bounds().1,
                        &gpu.device,
                        &gpu.queue,
                    );
                    true
                });
                self.record_runtime_command(
                    "SetScatterAxisVisibility",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterBackground { id, r, g, b } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.background_color = Some([r, g, b, 1.0]);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterBackground",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterLegend {
                id,
                visible,
                position,
                entries,
                title,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.legend.visible = visible;
                    rt.widget.chrome.legend.position = scatter::LegendPosition::from_str(&position);
                    rt.widget.chrome.legend.entries = entries
                        .into_iter()
                        .map(|(label, r, g, b)| scatter::LegendEntry {
                            label,
                            color: [r, g, b],
                        })
                        .collect();
                    rt.widget.chrome.legend.title = title;
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterLegend",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterScalarBar {
                id,
                visible,
                vmin,
                vmax,
                log_scale,
                colormap,
                title,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.scalar_bar.visible = visible;
                    rt.widget.chrome.scalar_bar.vmin = vmin;
                    rt.widget.chrome.scalar_bar.vmax = vmax;
                    rt.widget.chrome.scalar_bar.log_scale = log_scale;
                    rt.widget.chrome.scalar_bar.colormap = colormap;
                    rt.widget.chrome.scalar_bar.title = title;
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterScalarBar",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterOrientationAxes { id, visible } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.chrome.orientation_axes_visible = visible;
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterOrientationAxes",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::AddScatterLabel {
                id,
                label_id,
                x,
                y,
                z,
                text,
                r,
                g,
                b,
                size,
                anchor,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.add_user_label(
                        label_id,
                        glam::Vec3::new(x, y, z),
                        text,
                        [r, g, b],
                        size,
                        anchor,
                    );
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "AddScatterLabel",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::UpdateScatterLabel {
                id,
                label_id,
                x,
                y,
                z,
                text,
                r,
                g,
                b,
                size,
                anchor,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    let pos = match (x, y, z) {
                        (Some(px), Some(py), Some(pz)) => Some(glam::Vec3::new(px, py, pz)),
                        _ => None,
                    };
                    let color = match (r, g, b) {
                        (Some(cr), Some(cg), Some(cb)) => Some([cr, cg, cb]),
                        _ => None,
                    };
                    rt.widget
                        .update_user_label(label_id, pos, text, color, size, anchor);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "UpdateScatterLabel",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::RemoveScatterLabel { id, label_id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.remove_user_label(label_id);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "RemoveScatterLabel",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterLabelVisible {
                id,
                label_id,
                visible,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.set_user_label_visible(label_id, visible);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterLabelVisible",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::ClearScatterLabels { id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.clear_user_labels();
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "ClearScatterLabels",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::AddScatterLines {
                id,
                overlay_id,
                segments,
                r,
                g,
                b,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.add_line_overlay(overlay_id, segments, [r, g, b]);
                    rt.widget.refresh_user_lines(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "AddScatterLines",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::UpdateScatterLines {
                id,
                overlay_id,
                segments,
                r,
                g,
                b,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget
                        .update_line_overlay(overlay_id, segments, [r, g, b]);
                    rt.widget.refresh_user_lines(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "UpdateScatterLines",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::AddScatterBox {
                id,
                overlay_id,
                xmin,
                xmax,
                ymin,
                ymax,
                zmin,
                zmax,
                r,
                g,
                b,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.add_box_overlay(
                        overlay_id,
                        [xmin, xmax, ymin, ymax, zmin, zmax],
                        [r, g, b],
                    );
                    rt.widget.refresh_user_lines(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "AddScatterBox",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::RemoveScatterOverlay { id, overlay_id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.remove_line_overlay(overlay_id);
                    rt.widget.refresh_user_lines(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "RemoveScatterOverlay",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterOverlayVisible {
                id,
                overlay_id,
                visible,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.set_line_overlay_visible(overlay_id, visible);
                    rt.widget.refresh_user_lines(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterOverlayVisible",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::ClearScatterOverlays { id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.clear_line_overlays();
                    rt.widget.refresh_user_lines(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "ClearScatterOverlays",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::AddScatterActor {
                id,
                actor_id,
                payload_b64,
                colormap,
                payload_format,
                hover_meta,
                tooltip_axis_labels,
            } => {
                let result = decode_actor_payload(&payload_b64, &colormap, payload_format);
                match result {
                    Err(e) => self.record_runtime_command(
                        "AddScatterActor",
                        Some(id),
                        None,
                        None,
                        &format!("decode error: {e}"),
                        false,
                    ),
                    Ok(pts) => {
                        let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                            gpu.add_scatter_actor_points(
                                &id,
                                actor_id,
                                pts,
                                hover_meta.as_deref(),
                                &tooltip_axis_labels,
                            )
                        });
                        self.record_runtime_command(
                            "AddScatterActor",
                            Some(id),
                            None,
                            None,
                            if redraw {
                                "ok"
                            } else {
                                "no-op: scatter not found"
                            },
                            redraw,
                        )
                    }
                }
            }
            Command::AddScatterActorPacked {
                id,
                actor_id,
                payload,
                colormap,
                payload_format,
                hover_meta,
                tooltip_axis_labels,
            } => {
                let result = decode_actor_payload_bytes(&payload, &colormap, payload_format);
                match result {
                    Err(e) => self.record_runtime_command(
                        "AddScatterActorPacked",
                        Some(id),
                        None,
                        None,
                        &format!("decode error: {e}"),
                        false,
                    ),
                    Ok(pts) => {
                        let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                            gpu.add_scatter_actor_points(
                                &id,
                                actor_id,
                                pts,
                                hover_meta.as_deref(),
                                &tooltip_axis_labels,
                            )
                        });
                        self.record_runtime_command(
                            "AddScatterActorPacked",
                            Some(id),
                            None,
                            None,
                            if redraw {
                                "ok"
                            } else {
                                "no-op: scatter not found"
                            },
                            redraw,
                        )
                    }
                }
            }
            Command::UpdateScatterActor {
                id,
                actor_id,
                payload_b64,
                colormap,
                payload_format,
                tooltip_axis_labels,
            } => {
                let result = decode_actor_payload(&payload_b64, &colormap, payload_format);
                match result {
                    Err(e) => self.record_runtime_command(
                        "UpdateScatterActor",
                        Some(id),
                        None,
                        None,
                        &format!("decode error: {e}"),
                        false,
                    ),
                    Ok(pts) => {
                        let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                            gpu.update_scatter_actor_points(
                                &id,
                                actor_id,
                                pts,
                                &tooltip_axis_labels,
                            )
                        });
                        self.record_runtime_command(
                            "UpdateScatterActor",
                            Some(id),
                            None,
                            None,
                            if redraw {
                                "ok"
                            } else {
                                "no-op: scatter not found"
                            },
                            redraw,
                        )
                    }
                }
            }
            Command::UpdateScatterActorPacked {
                id,
                actor_id,
                payload,
                colormap,
                payload_format,
                tooltip_axis_labels,
            } => {
                let result = decode_actor_payload_bytes(&payload, &colormap, payload_format);
                match result {
                    Err(e) => self.record_runtime_command(
                        "UpdateScatterActorPacked",
                        Some(id),
                        None,
                        None,
                        &format!("decode error: {e}"),
                        false,
                    ),
                    Ok(pts) => {
                        let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                            gpu.update_scatter_actor_points(
                                &id,
                                actor_id,
                                pts,
                                &tooltip_axis_labels,
                            )
                        });
                        self.record_runtime_command(
                            "UpdateScatterActorPacked",
                            Some(id),
                            None,
                            None,
                            if redraw {
                                "ok"
                            } else {
                                "no-op: scatter not found"
                            },
                            redraw,
                        )
                    }
                }
            }
            Command::RemoveScatterActor { id, actor_id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.remove_actor(actor_id);
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "RemoveScatterActor",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterActorVisible {
                id,
                actor_id,
                visible,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.set_actor_visible(actor_id, visible);
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterActorVisible",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::ClearScatterActors { id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.clear_extra_actors();
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "ClearScatterActors",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::ClearScatterScene { id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    // Clear primary buffer.
                    rt.points.clear();
                    rt.data_min = glam::Vec3::ZERO;
                    rt.data_max = glam::Vec3::ZERO;
                    rt.widget.set_points(&gpu.device, &gpu.queue, &[]);
                    // Clear extra actors and streams.
                    rt.widget.clear_extra_actors();
                    // Clear user labels.
                    rt.widget.clear_user_labels();
                    // Clear line/box overlays.
                    rt.widget.clear_line_overlays();
                    // Clear meshes.
                    rt.widget.clear_mesh_actors();
                    // Clear hover state.
                    rt.widget.hover_label = None;
                    rt.primary_hover_meta.clear();
                    rt.primary_pick_cache = None;
                    // Clear transient selection/LOD state.
                    rt.widget.selection_rect = None;
                    rt.widget.selection_polygon = None;
                    rt.widget.lod_active = false;
                    // Refresh derived GPU state with zero bounds.
                    rt.widget.refresh_grid(
                        glam::Vec3::ZERO,
                        glam::Vec3::ZERO,
                        &gpu.device,
                        &gpu.queue,
                    );
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    rt.widget.refresh_user_lines(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "ClearScatterScene",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::AddScatterStream {
                id,
                actor_id,
                max_points,
                mode,
            } => {
                let stream_mode = if mode == "ring" {
                    scatter::StreamMode::Ring
                } else {
                    scatter::StreamMode::Append
                };
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget
                        .add_stream_actor(actor_id, max_points, stream_mode, &gpu.device);
                    true
                });
                self.record_runtime_command(
                    "AddScatterStream",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::StreamScatterActor {
                id,
                actor_id,
                payload_b64,
                colormap,
                payload_format,
            } => {
                let result = decode_actor_payload(&payload_b64, &colormap, payload_format);
                match result {
                    Err(e) => self.record_runtime_command(
                        "StreamScatterActor",
                        Some(id),
                        None,
                        None,
                        &format!("decode error: {e}"),
                        false,
                    ),
                    Ok(pts) => {
                        let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                            gpu.stream_scatter_actor_points(&id, actor_id, &pts)
                        });
                        self.record_runtime_command(
                            "StreamScatterActor",
                            Some(id),
                            None,
                            None,
                            if redraw {
                                "ok"
                            } else {
                                "no-op: scatter not found"
                            },
                            redraw,
                        )
                    }
                }
            }
            Command::StreamScatterActorPacked {
                id,
                actor_id,
                payload,
                colormap,
                payload_format,
            } => {
                let result = decode_actor_payload_bytes(&payload, &colormap, payload_format);
                match result {
                    Err(e) => self.record_runtime_command(
                        "StreamScatterActorPacked",
                        Some(id),
                        None,
                        None,
                        &format!("decode error: {e}"),
                        false,
                    ),
                    Ok(pts) => {
                        let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                            gpu.stream_scatter_actor_points(&id, actor_id, &pts)
                        });
                        self.record_runtime_command(
                            "StreamScatterActorPacked",
                            Some(id),
                            None,
                            None,
                            if redraw {
                                "ok"
                            } else {
                                "no-op: scatter not found"
                            },
                            redraw,
                        )
                    }
                }
            }
            Command::ClearScatterStream { id, actor_id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.clear_stream_actor(actor_id);
                    if let Some(actor) = rt.widget.extra_actors.get_mut(&actor_id) {
                        actor.pick_cache = None;
                    }
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "ClearScatterStream",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterLod {
                id,
                enabled,
                threshold,
                factor,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let device = &gpu.device;
                    let queue = &gpu.queue;
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.lod_enabled = enabled;
                    rt.widget.lod_threshold = threshold;
                    rt.widget.lod_factor = factor;
                    rt.metrics.last_lod_ms =
                        rt.widget.refresh_lod_buffers(&rt.points, device, queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterLod",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterPickingMode { id, mode } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.picking_mode = scatter::PickingMode::from_str(&mode);
                    true
                });
                self.record_runtime_command(
                    "SetScatterPickingMode",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterHoverTooltip { id, enabled } => {
                let mut needs_redraw = false;
                let ok = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.hover_tooltip_enabled = enabled;
                    if !enabled && rt.widget.hover_label.is_some() {
                        rt.widget.hover_label = None;
                        rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                        needs_redraw = true;
                    }
                    true
                });
                if needs_redraw {
                    self.request_redraw();
                }
                self.record_runtime_command(
                    "SetScatterHoverTooltip",
                    Some(id),
                    Some(enabled.to_string()),
                    None,
                    if ok { "ok" } else { "no-op: scatter not found" },
                    false,
                )
            }
            Command::AddScatterMesh {
                id,
                mesh_id,
                positions_b64,
                indices_b64,
                r,
                g,
                b,
                a,
                wireframe,
            } => {
                match (
                    decode_mesh_positions(&positions_b64),
                    decode_mesh_indices(&indices_b64),
                ) {
                    (Err(e), _) | (_, Err(e)) => self.record_runtime_command(
                        "AddScatterMesh",
                        Some(id),
                        None,
                        None,
                        &format!("decode error: {e}"),
                        false,
                    ),
                    (Ok(positions), Ok(indices)) => {
                        let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                            let Some(rt) = gpu.scatters.get_mut(&id) else {
                                return false;
                            };
                            rt.widget.add_mesh_actor(
                                mesh_id,
                                positions,
                                indices,
                                [r, g, b, a],
                                wireframe,
                                &gpu.device,
                            );
                            let (bmn, bmx) = rt.merged_bounds();
                            rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                            rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                            true
                        });
                        self.record_runtime_command(
                            "AddScatterMesh",
                            Some(id),
                            None,
                            None,
                            if redraw {
                                "ok"
                            } else {
                                "no-op: scatter not found"
                            },
                            redraw,
                        )
                    }
                }
            }
            Command::UpdateScatterMesh {
                id,
                mesh_id,
                positions_b64,
                indices_b64,
                r,
                g,
                b,
                a,
                wireframe,
            } => {
                let positions = match positions_b64.as_deref() {
                    Some(b) => match decode_mesh_positions(b) {
                        Ok(p) => Some(p),
                        Err(e) => {
                            return self.record_runtime_command(
                                "UpdateScatterMesh",
                                Some(id),
                                None,
                                None,
                                &format!("decode error: {e}"),
                                false,
                            )
                        }
                    },
                    None => None,
                };
                let indices = match indices_b64.as_deref() {
                    Some(b) => match decode_mesh_indices(b) {
                        Ok(i) => Some(i),
                        Err(e) => {
                            return self.record_runtime_command(
                                "UpdateScatterMesh",
                                Some(id),
                                None,
                                None,
                                &format!("decode error: {e}"),
                                false,
                            )
                        }
                    },
                    None => None,
                };
                let color = match (r, g, b, a) {
                    (Some(cr), Some(cg), Some(cb), Some(ca)) => Some([cr, cg, cb, ca]),
                    _ => None,
                };
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.update_mesh_actor(
                        mesh_id,
                        positions,
                        indices,
                        color,
                        wireframe,
                        &gpu.device,
                    );
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "UpdateScatterMesh",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::RemoveScatterMesh { id, mesh_id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.remove_mesh_actor(mesh_id);
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "RemoveScatterMesh",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterMeshVisible {
                id,
                mesh_id,
                visible,
            } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.set_mesh_actor_visible(mesh_id, visible);
                    true
                });
                self.record_runtime_command(
                    "SetScatterMeshVisible",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::ClearScatterMeshes { id } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.clear_mesh_actors();
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "ClearScatterMeshes",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::SetScatterParallelScale { id, half_w, half_h } => {
                let redraw = self.gpu.as_mut().is_some_and(|gpu| {
                    let Some(rt) = gpu.scatters.get_mut(&id) else {
                        return false;
                    };
                    rt.widget.set_parallel_scale(half_w, half_h, &gpu.queue);
                    let (bmn, bmx) = rt.merged_bounds();
                    rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                    rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                    true
                });
                self.record_runtime_command(
                    "SetScatterParallelScale",
                    Some(id),
                    None,
                    None,
                    if redraw {
                        "ok"
                    } else {
                        "no-op: scatter not found"
                    },
                    redraw,
                )
            }
            Command::ScatterScreenshot { id, request_id } => {
                let result = self.gpu.as_mut().and_then(|gpu| {
                    let rt = gpu.scatters.get_mut(&id)?;
                    match rt.widget.screenshot(&gpu.device, &gpu.queue) {
                        Ok((w, h, pixels)) => {
                            use base64::Engine as _;
                            let rgba_b64 = BASE64.encode(&pixels);
                            let json = format!(r#"{{"w":{w},"h":{h},"rgba_b64":"{rgba_b64}"}}"#);
                            Some(json)
                        }
                        Err(e) => {
                            eprintln!("DragonGUI: scatter screenshot error: {e}");
                            None
                        }
                    }
                });
                if let Some(bridge) = &self.command_bridge {
                    let json =
                        result.unwrap_or_else(|| r#"{"w":0,"h":0,"rgba_b64":""}"#.to_string());
                    bridge.complete_debug_snapshot(request_id, json);
                }
                self.record_runtime_command("ScatterScreenshot", Some(id), None, None, "ok", false)
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

    fn begin_scrollbar_drag(&mut self, hit: PanelScrollbarHit, pos: [f32; 2]) {
        self.scrollbar_drag = Some(ScrollbarDrag::new(hit, pos));
        self.update_scrollbar_drag(pos);
    }

    fn update_scrollbar_drag(&mut self, pos: [f32; 2]) {
        let Some(drag) = self.scrollbar_drag.as_ref().cloned() else {
            return;
        };
        let scroll = drag.compute_scroll(pos);
        let changed = self
            .gpu
            .as_mut()
            .is_some_and(|gpu| gpu.scroll_container_to_axis(&drag.widget_id, drag.axis, scroll));
        if changed {
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
                if let Some(gpu) = &mut self.gpu {
                    if let Some(tree) = gpu.widget_tree.as_mut() {
                        set_widget_checked_prop(tree, id, new_val);
                    }
                }
                self.emit_change(id, ChangeValue::Bool(new_val));
                needs_layout_rebuild = true;
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

    fn handle_scroll_container_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if self.modifiers.control_key() || self.modifiers.alt_key() || self.modifiers.super_key() {
            return false;
        }
        let Some(command) = scroll_keyboard_command(&event.logical_key) else {
            return false;
        };
        let Some(target) = self
            .gpu
            .as_ref()
            .and_then(|gpu| gpu.keyboard_scroll_container_target(self.last_mouse_pos))
        else {
            return false;
        };
        let Some((axis, scroll)) =
            scroll_keyboard_destination(&target, command, self.modifiers.shift_key())
        else {
            return false;
        };
        let changed = self
            .gpu
            .as_mut()
            .is_some_and(|gpu| gpu.scroll_container_to_axis(&target.id, axis, scroll));
        if changed {
            self.request_redraw();
        }
        true
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

        if self.handle_scroll_container_key(&event) {
            return;
        }

        let reset = matches!(
            event.physical_key,
            PhysicalKey::Code(KeyCode::KeyR) | PhysicalKey::Code(KeyCode::Home)
        );
        if reset {
            if let Some(gpu) = &mut self.gpu {
                // Reset the active scatter's camera, or the first visible scatter.
                let target_id = self
                    .active_scatter_id
                    .clone()
                    .or_else(|| gpu.visible_scatter_order.first().cloned());
                if let Some(id) = target_id {
                    if let Some(runtime) = gpu.scatters.get_mut(&id) {
                        runtime.widget.reset_camera(&gpu.queue);
                        self.request_redraw();
                    }
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
        let mut request_redraw = self.flush_deferred_popup_commands();
        let mut next_deadline = None;
        if let Some(gpu) = &mut self.gpu {
            let visual_dirty = gpu.expire_toasts()
                | gpu.tick_hover_transitions()
                | gpu.tick_focus_transitions()
                | gpu.tick_checked_transitions()
                | gpu.tick_active_transitions()
                | gpu.tick_open_transitions()
                | gpu.tick_selected_transitions()
                | gpu.tick_expanded_transitions()
                | gpu.tick_css_animations();
            if visual_dirty {
                gpu.rebuild_visuals();
                request_redraw = true;
            }
            next_deadline = gpu.next_toast_deadline();
            if gpu.has_style_transitions() || gpu.has_css_animations() {
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

            WindowEvent::ThemeChanged(theme) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.set_platform_color_scheme(winit_theme_color_scheme(theme));
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
                            let was_rect_select = self.rect_select_active;
                            let scatter_press = self.scatter_press_pos.take();
                            self.orbit_active = false;
                            self.rect_select_active = false;
                            let released_scrollbar = self.scrollbar_drag.take().is_some();
                            let released_slider =
                                self.slider_drag.as_ref().map(|drag| drag.widget_id.clone());
                            if let Some(id) = released_slider {
                                self.flush_slider_change(&id);
                            }
                            self.slider_drag = None;
                            if released_scrollbar {
                                self.request_redraw();
                                return;
                            }

                            if was_rect_select {
                                if let Some(scatter_id) = self.active_scatter_id.clone() {
                                    // Determine which selection mode was active.
                                    let mode = self
                                        .gpu
                                        .as_ref()
                                        .and_then(|g| g.scatters.get(&scatter_id))
                                        .map(|rt| rt.widget.picking_mode);

                                    if mode == Some(scatter::PickingMode::Lasso) {
                                        // Polygon selection.
                                        let poly = self
                                            .gpu
                                            .as_ref()
                                            .and_then(|g| g.scatters.get(&scatter_id))
                                            .and_then(|rt| rt.widget.selection_polygon.clone());
                                        if let Some(p) = poly {
                                            if let Some(payload) = self.gpu.as_ref().and_then(|g| {
                                                g.scatter_polygon_select_payload(&scatter_id, &p)
                                            }) {
                                                self.emit_change(
                                                    &scatter_id,
                                                    ChangeValue::Text(payload),
                                                );
                                            }
                                        }
                                        if let Some(gpu) = &mut self.gpu {
                                            if let Some(rt) = gpu.scatters.get_mut(&scatter_id) {
                                                rt.widget.selection_polygon = None;
                                                rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                            }
                                        }
                                    } else {
                                        // Rectangle selection.
                                        let rect = self
                                            .gpu
                                            .as_ref()
                                            .and_then(|g| g.scatters.get(&scatter_id))
                                            .and_then(|rt| rt.widget.selection_rect);
                                        if let Some(r) = rect {
                                            if let Some(payload) = self.gpu.as_ref().and_then(|g| {
                                                g.scatter_select_payload(&scatter_id, r)
                                            }) {
                                                self.emit_change(
                                                    &scatter_id,
                                                    ChangeValue::Text(payload),
                                                );
                                            }
                                        }
                                        if let Some(gpu) = &mut self.gpu {
                                            if let Some(rt) = gpu.scatters.get_mut(&scatter_id) {
                                                rt.widget.selection_rect = None;
                                                rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                            }
                                        }
                                    }
                                    self.request_redraw();
                                }
                            } else if was_orbiting {
                                let pos = self.last_mouse_pos.unwrap_or([0.0, 0.0]);
                                if let Some(scatter_id) = self.active_scatter_id.clone() {
                                    if let Some(gpu) = &mut self.gpu {
                                        if let Some(rt) = gpu.scatters.get_mut(&scatter_id) {
                                            rt.widget.lod_active = false;
                                            rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                        }
                                    }
                                    let moved2 = scatter_press
                                        .map(|start| {
                                            let dx = pos[0] - start[0];
                                            let dy = pos[1] - start[1];
                                            dx * dx + dy * dy
                                        })
                                        .unwrap_or(f32::INFINITY);
                                    if moved2 <= 16.0 {
                                        if let Some(payload) = self
                                            .gpu
                                            .as_mut()
                                            .and_then(|g| g.scatter_pick_payload(&scatter_id, pos))
                                        {
                                            self.emit_change(
                                                &scatter_id,
                                                ChangeValue::Text(payload),
                                            );
                                        }
                                    }
                                }
                                self.request_redraw();
                            }
                            if !self.orbit_active && !self.pan_active && !self.rect_select_active {
                                self.active_scatter_id = None;
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

                            if let Some(hit) = (!modal_active)
                                .then(|| self.gpu.as_ref().and_then(|g| g.panel_scrollbar_at(pos)))
                                .flatten()
                            {
                                self.set_focus(None);
                                if let Some(gpu) = &mut self.gpu {
                                    gpu.close_popups();
                                }
                                self.begin_scrollbar_drag(hit, pos);
                                self.request_redraw();
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

                            let scatter_id = self.gpu.as_ref().and_then(|g| g.scatter_at(pos));

                            if let Some(sid) = scatter_id {
                                self.set_focus(None);
                                self.scatter_press_pos = Some(pos);
                                self.active_scatter_id = Some(sid.clone());
                                let picking_mode = self
                                    .gpu
                                    .as_ref()
                                    .and_then(|g| g.scatters.get(&sid))
                                    .map(|rt| rt.widget.picking_mode)
                                    .unwrap_or(scatter::PickingMode::Point);
                                if picking_mode == scatter::PickingMode::Rectangle
                                    || picking_mode == scatter::PickingMode::Lasso
                                {
                                    // Rectangle / lasso: start selection drag
                                    let local_pos = self
                                        .gpu
                                        .as_ref()
                                        .and_then(|g| g.scatters.get(&sid))
                                        .map(|rt| {
                                            [
                                                pos[0] - rt.widget.offset[0],
                                                pos[1] - rt.widget.offset[1],
                                            ]
                                        })
                                        .unwrap_or(pos);
                                    if let Some(gpu) = &mut self.gpu {
                                        if let Some(rt) = gpu.scatters.get_mut(&sid) {
                                            if picking_mode == scatter::PickingMode::Lasso {
                                                rt.widget.selection_polygon = Some(vec![local_pos]);
                                                rt.widget.selection_rect = None;
                                            } else {
                                                rt.widget.selection_rect = Some([
                                                    local_pos[0],
                                                    local_pos[1],
                                                    local_pos[0],
                                                    local_pos[1],
                                                ]);
                                                rt.widget.selection_polygon = None;
                                            }
                                            rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                        }
                                    }
                                    self.orbit_active = false;
                                    self.rect_select_active = true;
                                } else if picking_mode == scatter::PickingMode::None {
                                    // Interaction disabled — consume the event but do nothing.
                                    self.orbit_active = false;
                                    self.rect_select_active = false;
                                } else {
                                    // PickingMode::Point — left drag orbits.
                                    self.orbit_active = true;
                                    self.rect_select_active = false;
                                    if let Some(gpu) = &mut self.gpu {
                                        if let Some(rt) = gpu.scatters.get_mut(&sid) {
                                            rt.widget.lod_active = true;
                                        }
                                    }
                                }
                            } else {
                                self.active_scatter_id = None;
                                self.set_focus(None);
                            }
                        }
                    }

                    MouseButton::Middle | MouseButton::Right => {
                        if !pressed {
                            let was_panning = self.pan_active;
                            self.pan_active = false;
                            if was_panning {
                                if let Some(sid) = self.active_scatter_id.clone() {
                                    if let Some(gpu) = &mut self.gpu {
                                        if let Some(rt) = gpu.scatters.get_mut(&sid) {
                                            rt.widget.lod_active = false;
                                            rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                        }
                                    }
                                    self.request_redraw();
                                }
                            }
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
                            let pan_scatter_id = self.gpu.as_ref().and_then(|g| g.scatter_at(pos));
                            self.pan_active = pan_scatter_id.is_some();
                            if let Some(ref sid) = pan_scatter_id {
                                self.active_scatter_id = pan_scatter_id.clone();
                                if let Some(gpu) = &mut self.gpu {
                                    if let Some(rt) = gpu.scatters.get_mut(sid) {
                                        rt.widget.lod_active = true;
                                    }
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }

            WindowEvent::CursorLeft { .. } => {
                self.last_mouse_pos = None;
                if let Some(gpu) = &mut self.gpu {
                    let old_hover = gpu.current_hover_id();
                    let requires_layout =
                        gpu.hover_change_requires_layout(old_hover.as_deref(), None);
                    let cleared = gpu.update_hover_state(None, None);
                    if cleared {
                        if requires_layout {
                            gpu.apply_layout();
                        } else {
                            gpu.rebuild_visuals();
                        }
                        self.request_redraw();
                    }
                }
                let mut stale_payloads: Vec<(String, String)> = Vec::new();
                let mut scatter_cleared = false;
                if let Some(gpu) = &mut self.gpu {
                    for (id, rt) in gpu.scatters.iter_mut() {
                        if rt.hover_tooltip_enabled && rt.widget.hover_label.is_some() {
                            rt.widget.hover_label = None;
                            rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                            scatter_cleared = true;
                            let p = json!({"event": "hover_changed", "widget_id": id}).to_string();
                            stale_payloads.push((id.clone(), p));
                        }
                    }
                }
                if scatter_cleared {
                    self.request_redraw();
                }
                for (id, payload) in stale_payloads {
                    self.emit_change(&id, ChangeValue::Text(payload));
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = [position.x as f32, position.y as f32];

                // Drag interactions take priority.
                if self.scrollbar_drag.is_some() {
                    self.update_scrollbar_drag(new_pos);
                } else if self.slider_drag.is_some() {
                    self.update_slider_drag(new_pos[0], false);
                } else if self.rect_select_active {
                    if let Some(sid) = self.active_scatter_id.clone() {
                        if let Some(gpu) = &mut self.gpu {
                            if let Some(rt) = gpu.scatters.get_mut(&sid) {
                                let local_pos = [
                                    new_pos[0] - rt.widget.offset[0],
                                    new_pos[1] - rt.widget.offset[1],
                                ];
                                if let Some(ref mut r) = rt.widget.selection_rect {
                                    r[2] = local_pos[0];
                                    r[3] = local_pos[1];
                                } else if let Some(ref mut poly) = rt.widget.selection_polygon {
                                    poly.push(local_pos);
                                }
                                rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                            }
                        }
                        self.request_redraw();
                    }
                } else if let Some(old) = self.last_mouse_pos {
                    let delta = glam::Vec2::new(new_pos[0] - old[0], new_pos[1] - old[1]);
                    if self.orbit_active || self.pan_active {
                        if let Some(sid) = self.active_scatter_id.clone() {
                            let mut cam_payload: Option<(String, String)> = None;
                            let mut needs_redraw = false;
                            if let Some(gpu) = &mut self.gpu {
                                if let Some(runtime) = gpu.scatters.get_mut(&sid) {
                                    if self.orbit_active {
                                        runtime.widget.camera.orbit(delta);
                                    } else if self.pan_active {
                                        runtime.widget.camera.pan(delta);
                                    }
                                    runtime.widget.update_camera(&gpu.queue);
                                    let (bmn, bmx) = runtime.merged_bounds();
                                    runtime
                                        .widget
                                        .refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                                    runtime.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                    needs_redraw = true;
                                    // Capture camera state for Python-side linked-camera propagation.
                                    let s = runtime.widget.camera.state();
                                    let payload = json!({
                                        "event": "camera_changed",
                                        "widget_id": sid,
                                        "camera": {
                                            "target": [s.target[0], s.target[1], s.target[2]],
                                            "distance": s.distance,
                                            "yaw": s.yaw,
                                            "pitch": s.pitch,
                                            "parallel": s.parallel,
                                        },
                                    })
                                    .to_string();
                                    cam_payload = Some((sid.clone(), payload));
                                }
                            }
                            if needs_redraw {
                                self.request_redraw();
                            }
                            if let Some((id, payload)) = cam_payload {
                                self.emit_change(&id, ChangeValue::Text(payload));
                            }
                        }
                    }
                }

                // Update hover state when no button is held.
                if self.scrollbar_drag.is_none()
                    && self.slider_drag.is_none()
                    && !self.orbit_active
                    && !self.pan_active
                    && !self.rect_select_active
                {
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
                            let requires_layout = gpu.hover_change_requires_layout(
                                old_hover.as_deref(),
                                new_hover.as_deref(),
                            );
                            gpu.update_hover_state(new_hover, new_dropdown_hover);
                            if requires_layout {
                                // Rich tooltip content participates in overlay layout, so those
                                // hover changes can affect rects as well as paint/text state.
                                gpu.apply_layout();
                            } else {
                                gpu.rebuild_visuals();
                            }
                        }
                        self.request_redraw();
                    }

                    // Clear stale hover labels on any scatter the cursor has left.
                    let hovered_sid = self.gpu.as_ref().and_then(|g| g.scatter_at(new_pos));
                    let mut stale_payloads: Vec<(String, String)> = Vec::new();
                    let mut stale_scatter_cleared = false;
                    if let Some(gpu) = &mut self.gpu {
                        let stale: Vec<String> = gpu
                            .scatters
                            .iter()
                            .filter(|(id, rt)| {
                                rt.hover_tooltip_enabled
                                    && rt.widget.hover_label.is_some()
                                    && hovered_sid.as_deref() != Some(id.as_str())
                            })
                            .map(|(id, _)| id.clone())
                            .collect();
                        for id in stale {
                            if let Some(rt) = gpu.scatters.get_mut(&id) {
                                rt.widget.hover_label = None;
                                rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                stale_scatter_cleared = true;
                            }
                            let p = json!({"event":"hover_changed","widget_id":&id}).to_string();
                            stale_payloads.push((id, p));
                        }
                    }
                    if stale_scatter_cleared {
                        self.request_redraw();
                    }
                    for (id, payload) in stale_payloads {
                        self.emit_change(&id, ChangeValue::Text(payload));
                    }

                    // Scatter hover tooltip: pick nearest point and show a floating label.
                    if let Some(sid) = hovered_sid {
                        let tooltip_enabled = self
                            .gpu
                            .as_ref()
                            .and_then(|g| g.scatters.get(&sid))
                            .map(|rt| rt.hover_tooltip_enabled)
                            .unwrap_or(false);
                        if tooltip_enabled {
                            let mut hover_payload: Option<(String, String)> = None;
                            let mut needs_redraw = false;
                            if let Some(gpu) = &mut self.gpu {
                                if let Some(rt) = gpu.scatters.get_mut(&sid) {
                                    let hit =
                                        rt.pick_all_actors_cached(new_pos[0], new_pos[1], 12.0);
                                    let new_label = hit.map(|(actor_id, idx, pt)| {
                                        let screen_x = new_pos[0] + 16.0;
                                        let screen_y = new_pos[1] - 8.0;
                                        let default_labels = &rt.tooltip_axis_labels;
                                        let labels = if actor_id == 0 {
                                            default_labels
                                        } else {
                                            rt.widget
                                                .extra_actors
                                                .get(&actor_id)
                                                .map(|a| &a.tooltip_axis_labels)
                                                .unwrap_or(default_labels)
                                        };
                                        let [lx, ly, lz] = labels;
                                        let coord_line = format!(
                                            "{}: {}\n{}: {}\n{}: {}",
                                            lx,
                                            format_4g(pt.position[0]),
                                            ly,
                                            format_4g(pt.position[1]),
                                            lz,
                                            format_4g(pt.position[2]),
                                        );
                                        let meta_text = if actor_id == 0 {
                                            rt.primary_hover_meta
                                                .get(idx)
                                                .filter(|s| !s.is_empty())
                                                .cloned()
                                        } else {
                                            rt.widget
                                                .extra_actors
                                                .get(&actor_id)
                                                .and_then(|a| a.hover_meta.get(idx))
                                                .filter(|s| !s.is_empty())
                                                .cloned()
                                        };
                                        let text = match meta_text {
                                            Some(fields) => format!("{coord_line}\n{fields}"),
                                            None => coord_line,
                                        };
                                        let payload = json!({
                                            "event": "hover_changed",
                                            "widget_id": &sid,
                                            "actor": actor_id,
                                            "index": idx,
                                            "x": pt.position[0],
                                            "y": pt.position[1],
                                            "z": pt.position[2],
                                            "hover_text": &text,
                                        })
                                        .to_string();
                                        (
                                            scatter::ProjectedLabel {
                                                screen_x,
                                                screen_y,
                                                text,
                                                is_title: false,
                                                color: None,
                                                font_size: None,
                                                anchor: "top-left".to_string(),
                                            },
                                            payload,
                                        )
                                    });
                                    let changed = match (&rt.widget.hover_label, &new_label) {
                                        (None, None) => false,
                                        (Some(_), None) | (None, Some(_)) => true,
                                        (Some(old), Some((new, _))) => {
                                            old.text != new.text
                                                || (old.screen_x - new.screen_x).abs() > 0.5
                                                || (old.screen_y - new.screen_y).abs() > 0.5
                                                || old.anchor != new.anchor
                                        }
                                    };
                                    if changed {
                                        if let Some((label, payload)) = new_label {
                                            rt.widget.hover_label = Some(label);
                                            hover_payload = Some((sid.clone(), payload));
                                        } else {
                                            rt.widget.hover_label = None;
                                            hover_payload = Some((
                                                sid.clone(),
                                                json!({"event":"hover_changed","widget_id":&sid})
                                                    .to_string(),
                                            ));
                                        }
                                        rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                        needs_redraw = true;
                                    }
                                }
                            }
                            if needs_redraw {
                                self.request_redraw();
                            }
                            if let Some((id, payload)) = hover_payload {
                                self.emit_change(&id, ChangeValue::Text(payload));
                            }
                        }
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
                    // Scatter zoom wins over parent scroll container when pointer is over the plot.
                    if let Some(sid) = self.gpu.as_ref().and_then(|gpu| gpu.scatter_at(pos)) {
                        let mut cam_payload: Option<(String, String)> = None;
                        let mut needs_redraw = false;
                        if let Some(gpu) = &mut self.gpu {
                            if let Some(rt) = gpu.scatters.get_mut(&sid) {
                                rt.widget.camera.zoom(scroll_y);
                                rt.widget.update_camera(&gpu.queue);
                                let (bmn, bmx) = rt.merged_bounds();
                                rt.widget.refresh_grid(bmn, bmx, &gpu.device, &gpu.queue);
                                rt.widget.refresh_overlays(&gpu.device, &gpu.queue);
                                needs_redraw = true;
                                let s = rt.widget.camera.state();
                                let payload = json!({
                                    "event": "camera_changed",
                                    "widget_id": sid,
                                    "camera": {
                                        "target": [s.target[0], s.target[1], s.target[2]],
                                        "distance": s.distance,
                                        "yaw": s.yaw,
                                        "pitch": s.pitch,
                                        "parallel": s.parallel,
                                    },
                                })
                                .to_string();
                                cam_payload = Some((sid.clone(), payload));
                            }
                        }
                        if needs_redraw {
                            self.request_redraw();
                        }
                        if let Some((id, payload)) = cam_payload {
                            self.emit_change(&id, ChangeValue::Text(payload));
                        }
                        return;
                    }
                    if let Some(id) = self
                        .gpu
                        .as_ref()
                        .and_then(|gpu| gpu.scroll_container_at(pos))
                    {
                        let container_scroll_x = if scroll_x.abs() >= 0.5 {
                            scroll_x
                        } else if self.modifiers.shift_key() {
                            scroll_y
                        } else {
                            0.0
                        };
                        let container_scroll_y =
                            if self.modifiers.shift_key() && scroll_x.abs() < 0.5 {
                                0.0
                            } else {
                                scroll_y
                            };
                        if self.gpu.as_mut().is_some_and(|gpu| {
                            gpu.scroll_container(&id, container_scroll_x, container_scroll_y)
                        }) {
                            self.request_redraw();
                        }
                        return;
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
