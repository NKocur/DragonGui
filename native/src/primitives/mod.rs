use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
    ops::Range,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use bytemuck::{Pod, Zeroable};
use serde_json::Value;

use crate::css_style::{
    computed_style_for_virtual_element_with_media, DgMediaEnvironment, StylesheetStore,
};
use crate::document::{BarChartHoverProp, HeatmapHoverProp, WidgetKind, WidgetNode};
use crate::events::{NavigationItem, SortDirection, TableSortColumn, WidgetState};
use crate::layout::{
    is_scroll_container_node, scroll_container_max_x, scroll_container_max_y,
    titled_container_geometry, tree_node_row_height_for_style, LayoutResult, Rect,
};
use crate::overlays::{
    dropdown_overlay_rect, find_node, menu_popup_rect, rich_tooltip_target, tooltip_target,
};
use crate::paint::{
    native_widget_paint_fallback_with_level, native_widget_part_paint_fallback,
    native_widget_part_paint_fallback_with_selection, NativePaintFallback, PaintInteraction,
};
use crate::scatter::colormap;
use crate::style::{
    base_part_style, checked_part_style_for_state, code_editor_gutter_width_for_style,
    collapsed_part_style_for_state, collapsible_header_height_for_style,
    expanded_part_style_for_state, inline_badge_layout_for_text,
    merged_part_visual_for_state as style_merged_part_visual_for_state,
    number_stepper_width_for_style, open_part_style_for_state,
    part_style_active_for_state as style_part_style_active_for_state,
    part_visual_for_state as style_part_visual_for_state, selected_part_style_for_state,
    state_part_style_for_state, tabs_header_height_for_style, uniform_layout_padding,
    BackdropFilterStyle, BackgroundPaint, BackgroundPatternKind, BorderLineStyle, ColorRef,
    GradientInterpolation, NodeStyle, PartStyle, PositionStyle, TextStyle, TransformStyle,
    TransitionProperty, VisualStyle, BORDER_WIDTH_LP, CARET_WIDTH_LP, CHECKBOX_BOX_LP,
    CHECKBOX_LEFT_PAD_LP, DROPDOWN_CHEVRON_WIDTH_LP, FOCUS_RING_LP, PANEL_ACCENT_WIDTH_LP,
    SLIDER_THUMB_WIDTH_LP, SLIDER_TRACK_HEIGHT_LP, SLIDER_TRACK_MARGIN_LP, TAB_ACTIVE_BAR_LP,
    TAB_GAP_LP, TAB_INACTIVE_BOTTOM_INSET_LP, TAB_TOP_INSET_LP, TOGGLE_SWITCH_THUMB_SIZE_LP,
    TOGGLE_SWITCH_TRACK_HEIGHT_LP, TOGGLE_SWITCH_TRACK_WIDTH_LP,
};
use crate::table;
use crate::text::measure_text_for_layout;
use crate::theme::{parse_hex_color, parse_web_color, Color, Theme};
use crate::toast::{toast_colors, toast_rect, toast_stack_index, ToastOverlay};

const SCROLLBAR_VISIBILITY_EPSILON_PX: f32 = 2.0;
const SCROLLBAR_MIN_TRACK_LEN_PX: f32 = 44.0;
const IMPLICIT_PANEL_SCROLLBAR_MIN_SIZE_PX: f32 = 64.0;
const LOADING_SPINNER_DEFAULT_SIZE_LP: f32 = 18.0;
const LOADING_SPINNER_TAU: f32 = std::f32::consts::PI * 2.0;

fn raw_prop_f32(node: &WidgetNode, name: &str) -> Option<f32> {
    node.props
        .raw_props
        .get(name)
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
}

fn raw_prop_bool(node: &WidgetNode, name: &str) -> Option<bool> {
    node.props
        .raw_props
        .get(name)
        .and_then(|value| value.as_bool())
}

fn value_f32(value: &Value) -> Option<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
}

fn object_f32(map: &serde_json::Map<String, Value>, name: &str) -> Option<f32> {
    map.get(name).and_then(value_f32)
}

fn normalize_color_channel(value: f32) -> f32 {
    if value > 1.0 {
        (value / 255.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn display_list_color(value: Option<&Value>, theme: &Theme, fallback: Color) -> Color {
    match value {
        Some(Value::String(text)) => parse_web_color(text)
            .or_else(|| parse_hex_color(text))
            .unwrap_or_else(|| ColorRef::Token(text.trim().to_string()).resolve(theme)),
        Some(Value::Array(items)) if items.len() == 3 || items.len() == 4 => {
            let r = items.first().and_then(value_f32).unwrap_or(0.0);
            let g = items.get(1).and_then(value_f32).unwrap_or(0.0);
            let b = items.get(2).and_then(value_f32).unwrap_or(0.0);
            let a = items.get(3).and_then(value_f32).unwrap_or(1.0);
            [
                normalize_color_channel(r),
                normalize_color_channel(g),
                normalize_color_channel(b),
                normalize_color_channel(a),
            ]
        }
        _ => fallback,
    }
}

fn display_list_scale(node: &WidgetNode, rect: [f32; 4]) -> (f32, f32) {
    let paint_w = node
        .props
        .raw_props
        .get("paint_width")
        .and_then(value_f32)
        .filter(|value| *value > 0.0)
        .or(node.props.intrinsic_width)
        .unwrap_or(rect[2].max(1.0));
    let paint_h = node
        .props
        .raw_props
        .get("paint_height")
        .and_then(value_f32)
        .filter(|value| *value > 0.0)
        .or(node.props.intrinsic_height)
        .unwrap_or(rect[3].max(1.0));
    (rect[2] / paint_w.max(1.0), rect[3] / paint_h.max(1.0))
}

fn emit_extension_display_list(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    rect: [f32; 4],
) {
    let Some(Value::Array(commands)) = node.props.raw_props.get("display_list") else {
        return;
    };
    let (sx, sy) = display_list_scale(node, rect);
    let stroke_scale = ((sx.abs() + sy.abs()) * 0.5).max(0.001);
    for command in commands {
        let Some(command) = command.as_object() else {
            continue;
        };
        let cmd = command
            .get("cmd")
            .or_else(|| command.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        match cmd {
            "rect" | "rounded_rect" => {
                let Some(local_x) = object_f32(command, "x") else {
                    continue;
                };
                let Some(local_y) = object_f32(command, "y") else {
                    continue;
                };
                let Some(local_w) =
                    object_f32(command, "w").or_else(|| object_f32(command, "width"))
                else {
                    continue;
                };
                let Some(local_h) =
                    object_f32(command, "h").or_else(|| object_f32(command, "height"))
                else {
                    continue;
                };
                if local_w <= 0.0 || local_h <= 0.0 {
                    continue;
                }
                let radius =
                    object_f32(command, "radius").unwrap_or(0.0).max(0.0) * sx.min(sy).abs();
                let screen = [
                    rect[0] + local_x * sx,
                    rect[1] + local_y * sy,
                    local_w * sx,
                    local_h * sy,
                ];
                if command.get("fill").is_some() {
                    let fill = display_list_color(command.get("fill"), theme, theme.surface_alt);
                    out.push(inst_radii(screen, fill, [radius; 4]));
                }
                if command.get("stroke").is_some() {
                    let stroke = display_list_color(command.get("stroke"), theme, theme.border);
                    let stroke_width = object_f32(command, "stroke_width")
                        .or_else(|| object_f32(command, "line_width"))
                        .unwrap_or(1.0)
                        .max(0.0)
                        * stroke_scale;
                    if stroke_width > 0.0 {
                        out.push(inst_outline_ring_clipped(
                            screen,
                            stroke,
                            [radius; 4],
                            stroke_width,
                            default_local_clip(screen),
                        ));
                    }
                }
            }
            "line" => {
                let Some(x1) = object_f32(command, "x1") else {
                    continue;
                };
                let Some(y1) = object_f32(command, "y1") else {
                    continue;
                };
                let Some(x2) = object_f32(command, "x2") else {
                    continue;
                };
                let Some(y2) = object_f32(command, "y2") else {
                    continue;
                };
                let start = [rect[0] + x1 * sx, rect[1] + y1 * sy];
                let end = [rect[0] + x2 * sx, rect[1] + y2 * sy];
                let stroke = display_list_color(command.get("stroke"), theme, theme.accent);
                let width = object_f32(command, "stroke_width")
                    .or_else(|| object_f32(command, "width"))
                    .unwrap_or(1.5)
                    .max(0.1)
                    * stroke_scale;
                if let Some((a, b)) = clip_line_segment_to_rect(start, end, rect) {
                    push_line_segment(out, a, b, width, stroke);
                }
            }
            "polyline" => {
                let Some(Value::Array(points)) = command.get("points") else {
                    continue;
                };
                if points.len() < 2 {
                    continue;
                }
                let stroke = display_list_color(command.get("stroke"), theme, theme.accent);
                let width = object_f32(command, "stroke_width")
                    .or_else(|| object_f32(command, "width"))
                    .unwrap_or(1.5)
                    .max(0.1)
                    * stroke_scale;
                let mut previous: Option<[f32; 2]> = None;
                for point in points {
                    let Some(items) = point.as_array() else {
                        previous = None;
                        continue;
                    };
                    if items.len() != 2 {
                        previous = None;
                        continue;
                    }
                    let Some(px) = items.first().and_then(value_f32) else {
                        previous = None;
                        continue;
                    };
                    let Some(py) = items.get(1).and_then(value_f32) else {
                        previous = None;
                        continue;
                    };
                    let current = [rect[0] + px * sx, rect[1] + py * sy];
                    if let Some(prev) = previous {
                        if let Some((a, b)) = clip_line_segment_to_rect(prev, current, rect) {
                            push_line_segment(out, a, b, width, stroke);
                        }
                    }
                    previous = Some(current);
                }
            }
            "circle" => {
                let Some(cx) = object_f32(command, "cx") else {
                    continue;
                };
                let Some(cy) = object_f32(command, "cy") else {
                    continue;
                };
                let Some(r) = object_f32(command, "r").or_else(|| object_f32(command, "radius"))
                else {
                    continue;
                };
                if r <= 0.0 {
                    continue;
                }
                let radius = r * sx.min(sy).abs();
                let screen = [
                    rect[0] + cx * sx - radius,
                    rect[1] + cy * sy - radius,
                    radius * 2.0,
                    radius * 2.0,
                ];
                if command.get("fill").is_some() {
                    let fill = display_list_color(command.get("fill"), theme, theme.accent);
                    out.push(inst_radii(screen, fill, [radius; 4]));
                }
                if command.get("stroke").is_some() {
                    let stroke = display_list_color(command.get("stroke"), theme, theme.border);
                    let stroke_width = object_f32(command, "stroke_width")
                        .or_else(|| object_f32(command, "line_width"))
                        .unwrap_or(1.0)
                        .max(0.0)
                        * stroke_scale;
                    if stroke_width > 0.0 {
                        out.push(inst_outline_ring_clipped(
                            screen,
                            stroke,
                            [radius; 4],
                            stroke_width,
                            default_local_clip(screen),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn modal_close_button_rect(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
) -> Option<[f32; 4]> {
    if node.kind != WidgetKind::Modal || !raw_prop_bool(node, "close_button").unwrap_or(false) {
        return None;
    }
    let rect = layout.rects.get(&node.id)?;
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return None;
    }
    let inset = (theme.spacing * sf * 0.75).max(6.0 * sf);
    let border_w = node
        .style
        .visual
        .border_width
        .unwrap_or(BORDER_WIDTH_LP)
        .max(0.0)
        * sf;
    let has_title = node
        .props
        .text
        .as_deref()
        .is_some_and(|text| !text.is_empty());
    let title_band_h = if has_title {
        titled_container_geometry(node, layout, sf, theme)
            .map(|geometry| geometry.title_band.h)
            .unwrap_or(0.0)
            .min((rect.h - border_w * 2.0).max(1.0))
    } else {
        26.0 * sf
    };
    let size = if has_title {
        (18.0 * sf).min((title_band_h - 6.0 * sf).max(12.0 * sf))
    } else {
        18.0 * sf
    }
    .min((rect.w - border_w * 2.0 - inset * 2.0).max(1.0))
    .min((rect.h - border_w * 2.0).max(1.0));
    let x = rect.x + rect.w - border_w - inset - size;
    let y = rect.y + border_w + ((title_band_h - size) * 0.5).max(0.0);
    Some([x, y, size, size])
}

fn loading_spinner_size_lp(node: &WidgetNode) -> f32 {
    raw_prop_f32(node, "size")
        .filter(|value| *value > 0.0)
        .unwrap_or(LOADING_SPINNER_DEFAULT_SIZE_LP)
}

fn loading_spinner_stroke_lp(node: &WidgetNode, size_lp: f32) -> f32 {
    raw_prop_f32(node, "stroke_width")
        .filter(|value| *value > 0.0)
        .unwrap_or_else(|| (size_lp * 0.14).clamp(1.75, 3.0))
}

fn loading_spinner_phase(node: &WidgetNode, disabled: bool) -> f32 {
    let spinning = raw_prop_bool(node, "spinning").unwrap_or(true) && !disabled;
    if !spinning {
        return -std::f32::consts::FRAC_PI_2;
    }
    let speed = raw_prop_f32(node, "speed")
        .filter(|value| *value >= 0.0)
        .unwrap_or(1.0);
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0);
    loading_spinner_phase_at_seconds(speed, seconds)
}

fn loading_spinner_phase_at_seconds(speed: f32, seconds: f64) -> f32 {
    let rotations = (seconds * speed as f64).rem_euclid(1.0) as f32;
    rotations * LOADING_SPINNER_TAU - std::f32::consts::FRAC_PI_2
}

// ---------------------------------------------------------------------------
// Per-instance GPU data
// ---------------------------------------------------------------------------

/// One rect drawn as a 6-vertex quad. Matches `RectInstance` in rect.wgsl.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct RectInstance {
    /// Pixel-space rect: x, y (top-left), w, h.
    pub rect: [f32; 4],
    /// RGBA linear colour.
    pub color: [f32; 4],
    /// Corner radii in pixels: top-left, top-right, bottom-right, bottom-left.
    pub radii: [f32; 4],
    /// Local clip bounds: left, top, right, bottom in rect-local pixels.
    pub clip: [f32; 4],
    /// x: edge softness, y: shape inset, z: shadow mode (1 outset, 2 inset), w: shape kind.
    pub params: [f32; 4],
    /// Secondary RGBA colour for gradient paints.
    pub color2: [f32; 4],
    /// x: paint kind, y/z: linear-gradient direction, w: gradient stop count or shape option.
    pub paint: [f32; 4],
    /// x/y: pixel translation, z/w: scale.
    pub transform: [f32; 4],
    /// x: rotation in radians around rect center.
    pub transform2: [f32; 4], // x rotation radians, y background noise strength
    /// Third RGBA colour for multi-stop gradient paints.
    pub color3: [f32; 4],
    /// Fourth RGBA colour for multi-stop gradient paints.
    pub color4: [f32; 4],
    /// Gradient stop positions for color, color2, color3, and color4.
    pub gradient_stops: [f32; 4],
    /// Fifth RGBA colour for richer gradient paints.
    pub color5: [f32; 4],
    /// Sixth RGBA colour for richer gradient paints.
    pub color6: [f32; 4],
    /// Additional gradient stop positions for color5 and color6.
    pub gradient_stops2: [f32; 4],
}

/// Compact instance for solid, axis-aligned rect fills.
///
/// This covers the common widget surface path without carrying the full
/// gradient/shadow/transform payload used by `RectInstance`.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SimpleRectInstance {
    /// Pixel-space rect: x, y (top-left), w, h.
    rect: [f32; 4],
    /// RGBA linear colour.
    color: [f32; 4],
    /// Corner radii in pixels: top-left, top-right, bottom-right, bottom-left.
    radii: [f32; 4],
    /// Local clip bounds: left, top, right, bottom in rect-local pixels.
    clip: [f32; 4],
}

/// Compact instance for solid transformed line/capsule segments.
///
/// `rect` is the unrotated local capsule bounds and `params.x` is the rotation
/// angle around the rect center. This covers LinePlot's segment path without
/// carrying the full gradient/shadow rectangle payload.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LineSegmentInstance {
    /// Pixel-space rect before rotation: x, y (top-left), length, width.
    rect: [f32; 4],
    /// RGBA linear colour.
    color: [f32; 4],
    /// x: rotation radians around rect center.
    params: [f32; 4],
    /// Absolute screen-space paint clip: left, top, right, bottom.
    clip: [f32; 4],
}

/// Raw line plot point stored in a compact GPU storage buffer.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LinePlotPointGpu {
    /// x/y: data point, z: cumulative screen-space path distance.
    data: [f32; 4],
}

/// Per-series draw metadata for the dedicated LinePlot renderer.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LinePlotSeriesInstance {
    /// Full plot rect used to map data coordinates to screen coordinates.
    plot: [f32; 4],
    /// Final paint clip rect in screen pixels.
    clip: [f32; 4],
    /// x_min, x_max, y_min, y_max.
    bounds: [f32; 4],
    /// RGBA line colour.
    color: [f32; 4],
    /// x: width px, y: point offset, z: point count.
    params: [f32; 4],
    /// x: line style code (0 solid, 1 dashed, 2 dotted, 3 dashdot).
    style: [f32; 4],
}

static RECT_ATTRS: [wgpu::VertexAttribute; 15] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 48,
        shader_location: 3,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 64,
        shader_location: 4,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 80,
        shader_location: 5,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 96,
        shader_location: 6,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 112,
        shader_location: 7,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 128,
        shader_location: 8,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 144,
        shader_location: 9,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 160,
        shader_location: 10,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 176,
        shader_location: 11,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 192,
        shader_location: 12,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 208,
        shader_location: 13,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 224,
        shader_location: 14,
    },
];

static SIMPLE_RECT_ATTRS: [wgpu::VertexAttribute; 4] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 48,
        shader_location: 3,
    },
];

static LINE_SEGMENT_ATTRS: [wgpu::VertexAttribute; 4] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 48,
        shader_location: 3,
    },
];

static LINE_PLOT_SERIES_ATTRS: [wgpu::VertexAttribute; 6] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 48,
        shader_location: 3,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 64,
        shader_location: 4,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 80,
        shader_location: 5,
    },
];

fn rect_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RectInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &RECT_ATTRS,
    }
}

fn simple_rect_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SimpleRectInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &SIMPLE_RECT_ATTRS,
    }
}

fn line_segment_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LineSegmentInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &LINE_SEGMENT_ATTRS,
    }
}

fn line_plot_series_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LinePlotSeriesInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &LINE_PLOT_SERIES_ATTRS,
    }
}

// ---------------------------------------------------------------------------
// Uniform block
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

// ---------------------------------------------------------------------------
// PrimitivesRenderer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
pub struct PrimitiveRendererStats {
    pub split_enabled: bool,
    pub split_collapsed: bool,
    pub rect_count: u32,
    pub simple_count: u32,
    pub line_count: u32,
    pub complex_count: u32,
    pub base_batches: u32,
    pub overlay_batches: u32,
    pub simple_batches: u32,
    pub line_batches: u32,
    pub complex_batches: u32,
    pub source_bytes: u64,
    pub simple_bytes: u64,
    pub line_bytes: u64,
    pub complex_bytes: u64,
    pub buffer_bytes: u64,
    pub last_emit_ms: f64,
    pub last_split_ms: f64,
    pub last_upload_ms: f64,
    pub full_rebuilds: u64,
    pub partial_base_attempts: u64,
    pub partial_base_rebuilds: u64,
    pub partial_base_fallbacks: u64,
    pub partial_buffer_patches: u64,
    pub partial_buffer_patch_fallbacks: u64,
    pub partial_upload_bytes: u64,
    pub last_partial_upload_bytes: u64,
    pub overlay_rebuilds: u64,
    pub targeted_line_plot_checks: u64,
    pub targeted_line_plot_rebuilds: u64,
    pub targeted_line_plot_skips: u64,
    pub icon_cache_capacity: u32,
    pub icon_cache_entries: u32,
    pub icon_cache_hits: u64,
    pub icon_cache_misses: u64,
    pub icon_cache_evictions: u64,
    pub icon_cache_parse_failures: u64,
    pub last_rebuild_partial_base: bool,
    pub last_rebuild_overlay_only: bool,
}

impl PrimitiveRendererStats {
    pub fn icon_geometry_cache_snapshot(self) -> Value {
        serde_json::json!({
            "capacity": self.icon_cache_capacity,
            "entries": self.icon_cache_entries,
            "hits": self.icon_cache_hits,
            "misses": self.icon_cache_misses,
            "evictions": self.icon_cache_evictions,
            "parse_failures": self.icon_cache_parse_failures,
        })
    }
}

const ICON_GEOMETRY_CACHE_CAPACITY: usize = 128;

#[derive(Clone, Debug, PartialEq)]
struct ParsedIconStroke {
    points: Vec<[f32; 2]>,
    closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct ParsedIconGeometry {
    view_box: [f32; 4],
    stroke_width: f32,
    strokes: Vec<ParsedIconStroke>,
}

#[derive(Debug)]
struct IconGeometryCache {
    capacity: usize,
    entries: HashMap<Value, Arc<ParsedIconGeometry>>,
    insertion_order: VecDeque<Value>,
    hits: u64,
    misses: u64,
    evictions: u64,
    parse_failures: u64,
}

impl Default for IconGeometryCache {
    fn default() -> Self {
        Self::with_capacity(ICON_GEOMETRY_CACHE_CAPACITY)
    }
}

impl IconGeometryCache {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
            hits: 0,
            misses: 0,
            evictions: 0,
            parse_failures: 0,
        }
    }

    fn resolve(&mut self, resource: &Value) -> Option<Arc<ParsedIconGeometry>> {
        if let Some(geometry) = self.entries.get(resource) {
            self.hits = self.hits.saturating_add(1);
            return Some(Arc::clone(geometry));
        }

        self.misses = self.misses.saturating_add(1);
        let Some(geometry) = parse_custom_icon_resource(resource) else {
            self.parse_failures = self.parse_failures.saturating_add(1);
            return None;
        };
        let geometry = Arc::new(geometry);
        if self.capacity == 0 {
            return Some(geometry);
        }
        while self.entries.len() >= self.capacity {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            if self.entries.remove(&oldest).is_some() {
                self.evictions = self.evictions.saturating_add(1);
            }
        }
        self.insertion_order.push_back(resource.clone());
        self.entries.insert(resource.clone(), Arc::clone(&geometry));
        Some(geometry)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrimitivePipelineKind {
    Simple,
    Line,
    Complex,
}

#[derive(Clone, Copy, Debug)]
struct PrimitiveRoute {
    kind: PrimitivePipelineKind,
    index: usize,
}

#[derive(Clone, Copy, Debug)]
struct PrimitiveBatch {
    kind: PrimitivePipelineKind,
    start: u32,
    count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrimitiveRebuildScope {
    Full,
    PartialBase,
    Overlay,
}

fn replace_retained_base_range(
    instances: &mut Vec<RectInstance>,
    ranges: &mut HashMap<String, Range<usize>>,
    overlay_start: &mut u32,
    widget_id: &str,
    replacement: Vec<RectInstance>,
) -> bool {
    let Some(old_range) = ranges.get(widget_id).cloned() else {
        return false;
    };
    if old_range.start >= old_range.end
        || old_range.end > *overlay_start as usize
        || old_range.end > instances.len()
    {
        return false;
    }
    let old_len = old_range.len();
    let new_len = replacement.len();
    instances.splice(old_range.clone(), replacement);
    let delta = new_len as isize - old_len as isize;
    let new_end = old_range.start + new_len;
    ranges.retain(|owner, range| {
        if owner == widget_id {
            *range = old_range.start..new_end;
        } else if range.start >= old_range.end {
            range.start = range.start.saturating_add_signed(delta);
            range.end = range.end.saturating_add_signed(delta);
        } else if range.start <= old_range.start && range.end >= old_range.end {
            range.end = range.end.saturating_add_signed(delta);
        } else if range.start >= old_range.start && range.end <= old_range.end {
            return false;
        }
        true
    });
    *overlay_start = (*overlay_start as usize)
        .saturating_add_signed(delta)
        .min(u32::MAX as usize) as u32;
    true
}

fn include_instance_index(range: &mut Option<Range<usize>>, index: usize) {
    match range {
        Some(range) => {
            range.start = range.start.min(index);
            range.end = range.end.max(index.saturating_add(1));
        }
        None => *range = Some(index..index.saturating_add(1)),
    }
}

fn write_instance_range<T: Pod>(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    instances: &[T],
    range: Option<Range<usize>>,
) -> u64 {
    let Some(range) = range else {
        return 0;
    };
    if range.is_empty() || range.end > instances.len() {
        return 0;
    }
    let stride = std::mem::size_of::<T>();
    let byte_offset = (range.start * stride) as u64;
    let bytes = bytemuck::cast_slice(&instances[range]);
    queue.write_buffer(buffer, byte_offset, bytes);
    bytes.len() as u64
}

pub struct PrimitivesRenderer {
    simple_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    complex_pipeline: wgpu::RenderPipeline,
    simple_vertex_buffer: wgpu::Buffer,
    simple_vertex_cap: u64,
    line_vertex_buffer: wgpu::Buffer,
    line_vertex_cap: u64,
    complex_vertex_buffer: wgpu::Buffer,
    complex_vertex_cap: u64,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: Vec<RectInstance>,
    simple_instances: Vec<SimpleRectInstance>,
    line_instances: Vec<LineSegmentInstance>,
    complex_instances: Vec<RectInstance>,
    source_routes: Vec<PrimitiveRoute>,
    base_batches: Vec<PrimitiveBatch>,
    overlay_batches: Vec<PrimitiveBatch>,
    base_subtree_ranges: HashMap<String, Range<usize>>,
    icon_geometry_cache: IconGeometryCache,
    split_enabled: bool,
    stats: PrimitiveRendererStats,
    pub rect_count: u32,
    overlay_start: u32,
}

impl PrimitivesRenderer {
    /// Stable, process-local signature of the retained primitive stream in
    /// paint order. Used only by targeted-vs-full rebuild diagnostics.
    pub(crate) fn diagnostic_signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.overlay_start.hash(&mut hasher);
        self.instances.len().hash(&mut hasher);
        hasher.write(bytemuck::cast_slice(&self.instances));
        hasher.finish()
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let complex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("prim-rect"),
            source: wgpu::ShaderSource::Wgsl(include_str!("rect.wgsl").into()),
        });
        let simple_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("prim-simple-rect"),
            source: wgpu::ShaderSource::Wgsl(include_str!("simple_rect.wgsl").into()),
        });
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("prim-line-segment"),
            source: wgpu::ShaderSource::Wgsl(include_str!("line_segment.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("prim-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("prim-pl"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let simple_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("prim-simple-rect-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &simple_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[simple_rect_instance_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &simple_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::DEPTH_STENCIL_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("prim-line-segment-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &line_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[line_segment_instance_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &line_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::DEPTH_STENCIL_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let complex_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("prim-rect-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &complex_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[rect_instance_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &complex_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::DEPTH_STENCIL_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prim-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("prim-bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let simple_initial_cap = (64 * std::mem::size_of::<SimpleRectInstance>()) as u64;
        let simple_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prim-simple-vb"),
            size: simple_initial_cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_initial_cap = (64 * std::mem::size_of::<LineSegmentInstance>()) as u64;
        let line_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prim-line-vb"),
            size: line_initial_cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let complex_initial_cap = (64 * std::mem::size_of::<RectInstance>()) as u64;
        let complex_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prim-complex-vb"),
            size: complex_initial_cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let split_enabled = std::env::var("DRAGONGUI_PRIMITIVE_SPLIT")
            .map(|value| {
                let value = value.trim().to_ascii_lowercase();
                !matches!(value.as_str(), "0" | "false" | "off" | "no")
            })
            .unwrap_or(true);

        let renderer = Self {
            simple_pipeline,
            line_pipeline,
            complex_pipeline,
            simple_vertex_buffer,
            simple_vertex_cap: simple_initial_cap,
            line_vertex_buffer,
            line_vertex_cap: line_initial_cap,
            complex_vertex_buffer,
            complex_vertex_cap: complex_initial_cap,
            uniform_buffer,
            bind_group,
            instances: Vec::with_capacity(64),
            simple_instances: Vec::with_capacity(64),
            line_instances: Vec::with_capacity(64),
            complex_instances: Vec::with_capacity(64),
            source_routes: Vec::with_capacity(64),
            base_batches: Vec::with_capacity(16),
            overlay_batches: Vec::with_capacity(4),
            base_subtree_ranges: HashMap::new(),
            icon_geometry_cache: IconGeometryCache::default(),
            split_enabled,
            stats: PrimitiveRendererStats::default(),
            rect_count: 0,
            overlay_start: 0,
        };
        renderer.update_screen_size(queue, width, height);
        renderer
    }

    /// Upload screen-size uniform (call on creation and every resize).
    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn upload_static_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: Vec<RectInstance>,
        overlay_start: u32,
    ) {
        self.instances = instances;
        self.rect_count = self.instances.len() as u32;
        self.overlay_start = overlay_start.min(self.rect_count);
        self.base_subtree_ranges.clear();
        self.split_and_upload(device, queue, 0.0, PrimitiveRebuildScope::Full);
    }

    /// Rebuild the instance list from layout, theme, and interactive state.
    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree: &WidgetNode,
        layout: &LayoutResult,
        theme: &Theme,
        scale_factor: f32,
        state: &WidgetState,
        caret_positions: &HashMap<String, [f32; 2]>,
        toasts: &[ToastOverlay],
        stylesheets: &StylesheetStore,
        media: DgMediaEnvironment,
    ) {
        let window_w = media.width * scale_factor;
        let window_h = media.height * scale_factor;
        self.instances.clear();
        self.base_subtree_ranges.clear();
        let emit_t0 = Instant::now();
        emit_rects_inner(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            caret_positions,
            true,
            RenderContext::default(),
            &mut self.instances,
            Some(&mut self.base_subtree_ranges),
            &mut self.icon_geometry_cache,
        );
        self.overlay_start = self.instances.len() as u32;
        emit_primitive_overlays(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            caret_positions,
            stylesheets,
            media,
            toasts,
            window_w,
            window_h,
            &mut self.instances,
            &mut self.icon_geometry_cache,
        );

        self.rect_count = self.instances.len() as u32;
        let emit_ms = emit_t0.elapsed().as_secs_f64() * 1000.0;
        self.split_and_upload(device, queue, emit_ms, PrimitiveRebuildScope::Full);
    }

    /// Rebuild only virtual overlay primitives while retaining the base-tree
    /// instance range and its established paint order.
    pub fn rebuild_overlays(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree: &WidgetNode,
        layout: &LayoutResult,
        theme: &Theme,
        scale_factor: f32,
        state: &WidgetState,
        caret_positions: &HashMap<String, [f32; 2]>,
        toasts: &[ToastOverlay],
        stylesheets: &StylesheetStore,
        media: DgMediaEnvironment,
    ) {
        let window_w = media.width * scale_factor;
        let window_h = media.height * scale_factor;
        let base_len = (self.overlay_start as usize).min(self.instances.len());
        self.instances.truncate(base_len);
        self.overlay_start = base_len as u32;
        let emit_t0 = Instant::now();
        emit_primitive_overlays(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            caret_positions,
            stylesheets,
            media,
            toasts,
            window_w,
            window_h,
            &mut self.instances,
            &mut self.icon_geometry_cache,
        );
        self.rect_count = self.instances.len() as u32;
        let emit_ms = emit_t0.elapsed().as_secs_f64() * 1000.0;
        self.split_and_upload(device, queue, emit_ms, PrimitiveRebuildScope::Overlay);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rebuild_base_subtrees(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree: &WidgetNode,
        layout: &LayoutResult,
        theme: &Theme,
        scale_factor: f32,
        state: &WidgetState,
        caret_positions: &HashMap<String, [f32; 2]>,
        toasts: &[ToastOverlay],
        stylesheets: &StylesheetStore,
        media: DgMediaEnvironment,
        widget_ids: &HashSet<String>,
        rebuild_overlays: bool,
    ) -> bool {
        if widget_ids.is_empty() {
            return false;
        }
        self.stats.partial_base_attempts = self.stats.partial_base_attempts.saturating_add(1);
        let mut replacements = Vec::with_capacity(widget_ids.len());
        for widget_id in widget_ids {
            let Some(range) = self.base_subtree_ranges.get(widget_id).cloned() else {
                return self.reject_partial_base_rebuild();
            };
            if range.end > self.overlay_start as usize {
                return self.reject_partial_base_rebuild();
            }
            let Some(node) = find_node(tree, widget_id) else {
                return self.reject_partial_base_rebuild();
            };
            let mut instances = Vec::with_capacity(range.len());
            let mut replacement_ranges = HashMap::new();
            emit_rects_inner(
                node,
                layout,
                theme,
                scale_factor,
                state,
                caret_positions,
                true,
                RenderContext::default(),
                &mut instances,
                Some(&mut replacement_ranges),
                &mut self.icon_geometry_cache,
            );
            if range.is_empty() {
                if instances.is_empty() {
                    continue;
                }
                return self.reject_partial_base_rebuild();
            }
            replacements.push((
                widget_id.clone(),
                range.start,
                range.len(),
                instances,
                replacement_ranges,
            ));
        }
        replacements.sort_by(|left, right| right.1.cmp(&left.1));

        let emit_t0 = Instant::now();
        let same_shape = replacements
            .iter()
            .all(|(_, _, old_len, replacement, _)| *old_len == replacement.len());
        let changed_ranges = replacements
            .iter()
            .map(|(_, range_start, _, replacement, _)| {
                *range_start..*range_start + replacement.len()
            })
            .collect::<Vec<_>>();
        for (widget_id, range_start, _, replacement, replacement_ranges) in replacements {
            let replaced = replace_retained_base_range(
                &mut self.instances,
                &mut self.base_subtree_ranges,
                &mut self.overlay_start,
                &widget_id,
                replacement,
            );
            debug_assert!(replaced, "validated retained primitive range");
            for (owner, range) in replacement_ranges {
                self.base_subtree_ranges
                    .insert(owner, range_start + range.start..range_start + range.end);
            }
        }

        if rebuild_overlays {
            let base_len = (self.overlay_start as usize).min(self.instances.len());
            self.instances.truncate(base_len);
            let window_w = media.width * scale_factor;
            let window_h = media.height * scale_factor;
            emit_primitive_overlays(
                tree,
                layout,
                theme,
                scale_factor,
                state,
                caret_positions,
                stylesheets,
                media,
                toasts,
                window_w,
                window_h,
                &mut self.instances,
                &mut self.icon_geometry_cache,
            );
        }
        self.rect_count = self.instances.len() as u32;
        let emit_ms = emit_t0.elapsed().as_secs_f64() * 1000.0;
        if !rebuild_overlays && same_shape {
            if self.patch_split_instances_and_upload(queue, &changed_ranges, emit_ms) {
                return true;
            }
            self.stats.partial_buffer_patch_fallbacks =
                self.stats.partial_buffer_patch_fallbacks.saturating_add(1);
        } else {
            self.stats.partial_buffer_patch_fallbacks =
                self.stats.partial_buffer_patch_fallbacks.saturating_add(1);
        }
        self.split_and_upload(device, queue, emit_ms, PrimitiveRebuildScope::PartialBase);
        true
    }

    pub fn record_targeted_line_plot_rebuild(&mut self, rebuilt: bool) {
        self.stats.targeted_line_plot_checks =
            self.stats.targeted_line_plot_checks.saturating_add(1);
        if rebuilt {
            self.stats.targeted_line_plot_rebuilds =
                self.stats.targeted_line_plot_rebuilds.saturating_add(1);
        } else {
            self.stats.targeted_line_plot_skips =
                self.stats.targeted_line_plot_skips.saturating_add(1);
        }
    }

    fn reject_partial_base_rebuild(&mut self) -> bool {
        self.stats.partial_base_fallbacks = self.stats.partial_base_fallbacks.saturating_add(1);
        false
    }

    fn patch_split_instances_and_upload(
        &mut self,
        queue: &wgpu::Queue,
        changed_ranges: &[Range<usize>],
        emit_ms: f64,
    ) -> bool {
        if self.source_routes.len() != self.instances.len() {
            return false;
        }

        let mut updates = Vec::new();
        for range in changed_ranges {
            if range.end > self.instances.len() {
                return false;
            }
            for source_index in range.clone() {
                let instance = self.instances[source_index];
                let route = self.source_routes[source_index];
                let expected_kind = pipeline_kind_for_instance(
                    &instance,
                    self.split_enabled,
                    self.stats.split_collapsed,
                );
                if route.kind != expected_kind {
                    return false;
                }
                let route_is_valid = match route.kind {
                    PrimitivePipelineKind::Simple => route.index < self.simple_instances.len(),
                    PrimitivePipelineKind::Line => route.index < self.line_instances.len(),
                    PrimitivePipelineKind::Complex => route.index < self.complex_instances.len(),
                };
                if !route_is_valid {
                    return false;
                }
                updates.push((route, instance));
            }
        }

        let upload_t0 = Instant::now();
        let mut simple_range = None;
        let mut line_range = None;
        let mut complex_range = None;
        for (route, instance) in updates {
            match route.kind {
                PrimitivePipelineKind::Simple => {
                    self.simple_instances[route.index] = SimpleRectInstance {
                        rect: instance.rect,
                        color: instance.color,
                        radii: instance.radii,
                        clip: instance.clip,
                    };
                    include_instance_index(&mut simple_range, route.index);
                }
                PrimitivePipelineKind::Line => {
                    self.line_instances[route.index] = line_segment_instance_from_rect(instance);
                    include_instance_index(&mut line_range, route.index);
                }
                PrimitivePipelineKind::Complex => {
                    self.complex_instances[route.index] = instance;
                    include_instance_index(&mut complex_range, route.index);
                }
            }
        }

        let mut upload_bytes = 0;
        upload_bytes += write_instance_range(
            queue,
            &self.simple_vertex_buffer,
            &self.simple_instances,
            simple_range,
        );
        upload_bytes += write_instance_range(
            queue,
            &self.line_vertex_buffer,
            &self.line_instances,
            line_range,
        );
        upload_bytes += write_instance_range(
            queue,
            &self.complex_vertex_buffer,
            &self.complex_instances,
            complex_range,
        );

        self.stats.last_emit_ms = emit_ms;
        self.stats.last_split_ms = 0.0;
        self.stats.last_upload_ms = upload_t0.elapsed().as_secs_f64() * 1000.0;
        self.stats.partial_base_rebuilds = self.stats.partial_base_rebuilds.saturating_add(1);
        self.stats.partial_buffer_patches = self.stats.partial_buffer_patches.saturating_add(1);
        self.stats.partial_upload_bytes =
            self.stats.partial_upload_bytes.saturating_add(upload_bytes);
        self.stats.last_partial_upload_bytes = upload_bytes;
        self.stats.last_rebuild_partial_base = true;
        self.stats.last_rebuild_overlay_only = false;
        true
    }

    fn split_and_upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        emit_ms: f64,
        scope: PrimitiveRebuildScope,
    ) {
        let split_t0 = Instant::now();
        self.simple_instances.clear();
        self.line_instances.clear();
        self.complex_instances.clear();
        self.source_routes.clear();
        self.base_batches.clear();
        self.overlay_batches.clear();

        for (index, instance) in self.instances.iter().copied().enumerate() {
            let overlay = index >= self.overlay_start as usize;
            let batches = if overlay {
                &mut self.overlay_batches
            } else {
                &mut self.base_batches
            };
            let kind = pipeline_kind_for_instance(&instance, self.split_enabled, false);
            match kind {
                PrimitivePipelineKind::Line => {
                    let start = self.line_instances.len() as u32;
                    self.source_routes.push(PrimitiveRoute {
                        kind,
                        index: start as usize,
                    });
                    self.line_instances
                        .push(line_segment_instance_from_rect(instance));
                    push_primitive_batch(batches, kind, start, 1);
                }
                PrimitivePipelineKind::Simple => {
                    let start = self.simple_instances.len() as u32;
                    self.source_routes.push(PrimitiveRoute {
                        kind,
                        index: start as usize,
                    });
                    self.simple_instances.push(SimpleRectInstance {
                        rect: instance.rect,
                        color: instance.color,
                        radii: instance.radii,
                        clip: instance.clip,
                    });
                    push_primitive_batch(batches, kind, start, 1);
                }
                PrimitivePipelineKind::Complex => {
                    let start = self.complex_instances.len() as u32;
                    self.source_routes.push(PrimitiveRoute {
                        kind,
                        index: start as usize,
                    });
                    self.complex_instances.push(instance);
                    push_primitive_batch(batches, kind, start, 1);
                }
            }
        }
        let mut split_collapsed = false;
        if self.split_enabled
            && should_collapse_split_batches(
                self.instances.len(),
                self.batch_count(),
                self.line_instances.len(),
            )
        {
            split_collapsed = true;
            self.rebuild_as_complex_batches();
        }
        let split_ms = split_t0.elapsed().as_secs_f64() * 1000.0;

        let upload_t0 = Instant::now();
        let simple_size =
            (self.simple_instances.len() * std::mem::size_of::<SimpleRectInstance>()) as u64;
        if simple_size > self.simple_vertex_cap {
            let cap = (simple_size * 2).max(4096);
            self.simple_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("prim-simple-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.simple_vertex_cap = cap;
        }
        if !self.simple_instances.is_empty() {
            queue.write_buffer(
                &self.simple_vertex_buffer,
                0,
                bytemuck::cast_slice(&self.simple_instances),
            );
        }

        let line_size =
            (self.line_instances.len() * std::mem::size_of::<LineSegmentInstance>()) as u64;
        if line_size > self.line_vertex_cap {
            let cap = (line_size * 2).max(4096);
            self.line_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("prim-line-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.line_vertex_cap = cap;
        }
        if !self.line_instances.is_empty() {
            queue.write_buffer(
                &self.line_vertex_buffer,
                0,
                bytemuck::cast_slice(&self.line_instances),
            );
        }

        let complex_size =
            (self.complex_instances.len() * std::mem::size_of::<RectInstance>()) as u64;
        if complex_size > self.complex_vertex_cap {
            let cap = (complex_size * 2).max(4096);
            self.complex_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("prim-complex-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.complex_vertex_cap = cap;
        }
        if !self.complex_instances.is_empty() {
            queue.write_buffer(
                &self.complex_vertex_buffer,
                0,
                bytemuck::cast_slice(&self.complex_instances),
            );
        }
        let upload_ms = upload_t0.elapsed().as_secs_f64() * 1000.0;

        let simple_batches = self
            .base_batches
            .iter()
            .chain(self.overlay_batches.iter())
            .filter(|batch| batch.kind == PrimitivePipelineKind::Simple)
            .count() as u32;
        let line_batches = self
            .base_batches
            .iter()
            .chain(self.overlay_batches.iter())
            .filter(|batch| batch.kind == PrimitivePipelineKind::Line)
            .count() as u32;
        let complex_batches = self
            .base_batches
            .iter()
            .chain(self.overlay_batches.iter())
            .filter(|batch| batch.kind == PrimitivePipelineKind::Complex)
            .count() as u32;
        let full_rebuilds = self
            .stats
            .full_rebuilds
            .saturating_add(u64::from(scope == PrimitiveRebuildScope::Full));
        let partial_base_rebuilds = self
            .stats
            .partial_base_rebuilds
            .saturating_add(u64::from(scope == PrimitiveRebuildScope::PartialBase));
        let partial_base_attempts = self.stats.partial_base_attempts;
        let partial_base_fallbacks = self.stats.partial_base_fallbacks;
        let partial_buffer_patches = self.stats.partial_buffer_patches;
        let partial_buffer_patch_fallbacks = self.stats.partial_buffer_patch_fallbacks;
        let partial_upload_bytes = self.stats.partial_upload_bytes;
        let targeted_line_plot_checks = self.stats.targeted_line_plot_checks;
        let targeted_line_plot_rebuilds = self.stats.targeted_line_plot_rebuilds;
        let targeted_line_plot_skips = self.stats.targeted_line_plot_skips;
        let icon_cache_capacity = self.icon_geometry_cache.capacity as u32;
        let icon_cache_entries = self.icon_geometry_cache.entries.len() as u32;
        let icon_cache_hits = self.icon_geometry_cache.hits;
        let icon_cache_misses = self.icon_geometry_cache.misses;
        let icon_cache_evictions = self.icon_geometry_cache.evictions;
        let icon_cache_parse_failures = self.icon_geometry_cache.parse_failures;
        let overlay_rebuilds = self
            .stats
            .overlay_rebuilds
            .saturating_add(u64::from(scope == PrimitiveRebuildScope::Overlay));
        self.stats = PrimitiveRendererStats {
            split_enabled: self.split_enabled,
            split_collapsed,
            rect_count: self.instances.len() as u32,
            simple_count: self.simple_instances.len() as u32,
            line_count: self.line_instances.len() as u32,
            complex_count: self.complex_instances.len() as u32,
            base_batches: self.base_batches.len() as u32,
            overlay_batches: self.overlay_batches.len() as u32,
            simple_batches,
            line_batches,
            complex_batches,
            source_bytes: (self.instances.len() * std::mem::size_of::<RectInstance>()) as u64,
            simple_bytes: simple_size,
            line_bytes: line_size,
            complex_bytes: complex_size,
            buffer_bytes: simple_size + line_size + complex_size,
            last_emit_ms: emit_ms,
            last_split_ms: split_ms,
            last_upload_ms: upload_ms,
            full_rebuilds,
            partial_base_attempts,
            partial_base_rebuilds,
            partial_base_fallbacks,
            partial_buffer_patches,
            partial_buffer_patch_fallbacks,
            partial_upload_bytes,
            last_partial_upload_bytes: 0,
            overlay_rebuilds,
            targeted_line_plot_checks,
            targeted_line_plot_rebuilds,
            targeted_line_plot_skips,
            icon_cache_capacity,
            icon_cache_entries,
            icon_cache_hits,
            icon_cache_misses,
            icon_cache_evictions,
            icon_cache_parse_failures,
            last_rebuild_partial_base: scope == PrimitiveRebuildScope::PartialBase,
            last_rebuild_overlay_only: scope == PrimitiveRebuildScope::Overlay,
        };
    }

    fn batch_count(&self) -> usize {
        self.base_batches.len() + self.overlay_batches.len()
    }

    fn rebuild_as_complex_batches(&mut self) {
        self.simple_instances.clear();
        self.line_instances.clear();
        self.complex_instances.clear();
        self.source_routes.clear();
        self.base_batches.clear();
        self.overlay_batches.clear();
        for (index, instance) in self.instances.iter().copied().enumerate() {
            let start = self.complex_instances.len() as u32;
            self.source_routes.push(PrimitiveRoute {
                kind: PrimitivePipelineKind::Complex,
                index: start as usize,
            });
            self.complex_instances.push(instance);
            let batches = if index >= self.overlay_start as usize {
                &mut self.overlay_batches
            } else {
                &mut self.base_batches
            };
            push_primitive_batch(batches, PrimitivePipelineKind::Complex, start, 1);
        }
    }

    pub fn render_base(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.rect_count == 0 {
            return;
        }
        let count = self.overlay_start.min(self.rect_count);
        if count == 0 {
            return;
        }
        self.render_batches(pass, &self.base_batches);
    }

    pub fn render_overlays(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.rect_count == 0 || self.overlay_start >= self.rect_count {
            return;
        }
        self.render_batches(pass, &self.overlay_batches);
    }

    fn render_batches(&self, pass: &mut wgpu::RenderPass<'_>, batches: &[PrimitiveBatch]) {
        let mut current_kind = None;
        for batch in batches {
            if batch.count == 0 {
                continue;
            }
            if current_kind != Some(batch.kind) {
                match batch.kind {
                    PrimitivePipelineKind::Simple => {
                        pass.set_pipeline(&self.simple_pipeline);
                        pass.set_bind_group(0, &self.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.simple_vertex_buffer.slice(..));
                    }
                    PrimitivePipelineKind::Line => {
                        pass.set_pipeline(&self.line_pipeline);
                        pass.set_bind_group(0, &self.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.line_vertex_buffer.slice(..));
                    }
                    PrimitivePipelineKind::Complex => {
                        pass.set_pipeline(&self.complex_pipeline);
                        pass.set_bind_group(0, &self.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.complex_vertex_buffer.slice(..));
                    }
                }
                current_kind = Some(batch.kind);
            }
            pass.draw(0..6, batch.start..batch.start + batch.count);
        }
    }

    pub fn stats(&self) -> PrimitiveRendererStats {
        self.stats
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LinePlotRendererStats {
    pub enabled: bool,
    pub aa_width: f32,
    pub max_segments_per_series: u32,
    pub decimation_mode: u32,
    pub series_count: u32,
    pub source_point_count: u32,
    pub decimated_series_count: u32,
    pub point_count: u32,
    pub segment_count: u32,
    pub point_bytes: u64,
    pub series_bytes: u64,
    pub buffer_bytes: u64,
    pub last_decimate_ms: f64,
    pub last_emit_ms: f64,
    pub last_upload_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinePlotDecimationMode {
    Off,
    Auto,
    Extrema,
}

impl LinePlotDecimationMode {
    fn metric_code(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Auto => 1,
            Self::Extrema => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LinePlotRenderBuildStats {
    source_point_count: usize,
    decimated_series_count: u32,
    decimate_ms: f64,
}

pub struct LinePlotRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    point_buffer: wgpu::Buffer,
    point_buffer_cap: u64,
    series_buffer: wgpu::Buffer,
    series_buffer_cap: u64,
    points: Vec<LinePlotPointGpu>,
    series_instances: Vec<LinePlotSeriesInstance>,
    enabled: bool,
    stats: LinePlotRendererStats,
}

impl LinePlotRenderer {
    /// Stable, process-local signature of retained line-plot GPU inputs.
    pub(crate) fn diagnostic_signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.enabled.hash(&mut hasher);
        self.points.len().hash(&mut hasher);
        hasher.write(bytemuck::cast_slice(&self.points));
        self.series_instances.len().hash(&mut hasher);
        hasher.write(bytemuck::cast_slice(&self.series_instances));
        hasher.finish()
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("line-plot-renderer"),
            source: wgpu::ShaderSource::Wgsl(include_str!("line_plot.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("line-plot-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("line-plot-pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line-plot-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[line_plot_series_instance_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::DEPTH_STENCIL_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line-plot-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_buffer_cap = 4096;
        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line-plot-points"),
            size: point_buffer_cap,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let series_buffer_cap = 1024;
        let series_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line-plot-series"),
            size: series_buffer_cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group =
            Self::make_bind_group(device, &bind_group_layout, &uniform_buffer, &point_buffer);

        let renderer = Self {
            pipeline,
            bind_group_layout,
            bind_group,
            uniform_buffer,
            point_buffer,
            point_buffer_cap,
            series_buffer,
            series_buffer_cap,
            points: Vec::with_capacity(4096),
            series_instances: Vec::with_capacity(8),
            enabled: line_plot_gpu_renderer_enabled(),
            stats: LinePlotRendererStats::default(),
        };
        renderer.update_screen_size(queue, width, height);
        renderer
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
        point_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("line-plot-bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: point_buffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree: &WidgetNode,
        layout: &LayoutResult,
        theme: &Theme,
        scale_factor: f32,
    ) {
        self.enabled = line_plot_gpu_renderer_enabled();
        self.points.clear();
        self.series_instances.clear();

        let aa_width = line_plot_aa_width();
        let max_segments_per_series = line_plot_renderer_max_segments_per_series();
        let decimation_mode = line_plot_decimation_mode();
        let mut build_stats = LinePlotRenderBuildStats::default();
        let emit_t0 = Instant::now();
        if self.enabled {
            collect_line_plot_render_data(
                tree,
                layout,
                theme,
                scale_factor,
                aa_width,
                max_segments_per_series,
                decimation_mode,
                None,
                &mut self.points,
                &mut self.series_instances,
                &mut build_stats,
            );
        }
        let emit_ms = emit_t0.elapsed().as_secs_f64() * 1000.0;

        let upload_t0 = Instant::now();
        let point_size = (self.points.len() * std::mem::size_of::<LinePlotPointGpu>()) as u64;
        if point_size > self.point_buffer_cap {
            let cap = (point_size * 2).max(4096);
            self.point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line-plot-points"),
                size: cap,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.point_buffer_cap = cap;
            self.bind_group = Self::make_bind_group(
                device,
                &self.bind_group_layout,
                &self.uniform_buffer,
                &self.point_buffer,
            );
        }
        if !self.points.is_empty() {
            queue.write_buffer(&self.point_buffer, 0, bytemuck::cast_slice(&self.points));
        }

        let series_size =
            (self.series_instances.len() * std::mem::size_of::<LinePlotSeriesInstance>()) as u64;
        if series_size > self.series_buffer_cap {
            let cap = (series_size * 2).max(1024);
            self.series_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("line-plot-series"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.series_buffer_cap = cap;
        }
        if !self.series_instances.is_empty() {
            queue.write_buffer(
                &self.series_buffer,
                0,
                bytemuck::cast_slice(&self.series_instances),
            );
        }
        let upload_ms = upload_t0.elapsed().as_secs_f64() * 1000.0;

        let segment_count = self
            .series_instances
            .iter()
            .map(|series| (series.params[2].round().max(0.0) as u32).saturating_sub(1))
            .sum();
        self.stats = LinePlotRendererStats {
            enabled: self.enabled,
            aa_width,
            max_segments_per_series: max_segments_per_series as u32,
            decimation_mode: decimation_mode.metric_code(),
            series_count: self.series_instances.len() as u32,
            source_point_count: build_stats.source_point_count as u32,
            decimated_series_count: build_stats.decimated_series_count,
            point_count: self.points.len() as u32,
            segment_count,
            point_bytes: point_size,
            series_bytes: series_size,
            buffer_bytes: point_size + series_size,
            last_decimate_ms: build_stats.decimate_ms,
            last_emit_ms: emit_ms,
            last_upload_ms: upload_ms,
        };
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.enabled || self.series_instances.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.series_buffer.slice(..));
        for (index, series) in self.series_instances.iter().enumerate() {
            let point_count = series.params[2].round().max(0.0) as u32;
            let vertex_count = point_count.saturating_sub(1) * 6;
            if vertex_count == 0 {
                continue;
            }
            let instance = index as u32;
            pass.draw(0..vertex_count, instance..instance + 1);
        }
    }

    pub fn stats(&self) -> LinePlotRendererStats {
        self.stats
    }
}

pub(crate) fn line_plot_gpu_renderer_enabled() -> bool {
    std::env::var("DRAGONGUI_LINE_PLOT_RENDERER")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "0" | "false" | "off" | "no")
        })
        .unwrap_or(true)
}

pub(crate) fn line_plot_aa_width() -> f32 {
    std::env::var("DRAGONGUI_LINE_PLOT_AA_WIDTH")
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(1.0)
        .clamp(0.5, 2.5)
}

pub(crate) fn line_plot_renderer_max_segments_per_series() -> usize {
    std::env::var("DRAGONGUI_LINE_PLOT_MAX_SEGMENTS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(8192)
        .clamp(1024, 65_536)
}

fn line_plot_decimation_mode() -> LinePlotDecimationMode {
    std::env::var("DRAGONGUI_LINE_PLOT_DECIMATION")
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "off" | "no" | "exact" | "none" => LinePlotDecimationMode::Off,
            "extrema" | "minmax" | "m4" | "preserve-extrema" | "preserve_extrema" => {
                LinePlotDecimationMode::Extrema
            }
            _ => LinePlotDecimationMode::Auto,
        })
        .unwrap_or(LinePlotDecimationMode::Auto)
}

fn line_plot_renderer_style_code(line_style: &str) -> Option<f32> {
    match line_style {
        "" | "solid" => Some(0.0),
        "dashed" => Some(1.0),
        "dotted" => Some(2.0),
        "dashdot" => Some(3.0),
        _ => None,
    }
}

fn push_primitive_batch(
    batches: &mut Vec<PrimitiveBatch>,
    kind: PrimitivePipelineKind,
    start: u32,
    count: u32,
) {
    if let Some(last) = batches.last_mut() {
        if last.kind == kind && last.start + last.count == start {
            last.count += count;
            return;
        }
    }
    batches.push(PrimitiveBatch { kind, start, count });
}

fn is_simple_rect_instance(instance: &RectInstance) -> bool {
    const EPS: f32 = 1.0e-5;
    instance.rect[2] > 0.0
        && instance.rect[3] > 0.0
        && (instance.params[0] - 1.0).abs() <= EPS
        && instance.params[1].abs() <= EPS
        && instance.params[2].abs() <= EPS
        && instance.params[3].abs() <= EPS
        && instance.paint[0].abs() <= EPS
        && instance.transform[0].abs() <= EPS
        && instance.transform[1].abs() <= EPS
        && (instance.transform[2] - 1.0).abs() <= EPS
        && (instance.transform[3] - 1.0).abs() <= EPS
        && instance.transform2[0].abs() <= EPS
        && instance.transform2[1].abs() <= EPS
        && instance.transform2[2].abs() <= EPS
}

fn is_line_segment_instance(instance: &RectInstance) -> bool {
    const EPS: f32 = 1.0e-5;
    if instance.rect[2] <= 0.0
        || instance.rect[3] <= 0.0
        || (instance.transform2[3] - 1.0).abs() > EPS
    {
        return false;
    }
    instance.params[0].is_finite()
        && instance.rect.iter().all(|value| value.is_finite())
        && instance.color.iter().all(|value| value.is_finite())
        && (instance.params[0] - 1.0).abs() <= EPS
        && instance.params[1].abs() <= EPS
        && instance.params[2].abs() <= EPS
        && instance.params[3].abs() <= EPS
        && instance.paint[0].abs() <= EPS
        && instance.transform[0].abs() <= EPS
        && instance.transform[1].abs() <= EPS
        && (instance.transform[2] - 1.0).abs() <= EPS
        && (instance.transform[3] - 1.0).abs() <= EPS
        && instance.transform2[1].abs() <= EPS
        && instance.transform2[2].abs() <= EPS
}

fn pipeline_kind_for_instance(
    instance: &RectInstance,
    split_enabled: bool,
    split_collapsed: bool,
) -> PrimitivePipelineKind {
    if split_collapsed || !split_enabled {
        PrimitivePipelineKind::Complex
    } else if is_line_segment_instance(instance) {
        PrimitivePipelineKind::Line
    } else if is_simple_rect_instance(instance) {
        PrimitivePipelineKind::Simple
    } else {
        PrimitivePipelineKind::Complex
    }
}

fn line_segment_instance_from_rect(instance: RectInstance) -> LineSegmentInstance {
    LineSegmentInstance {
        rect: instance.rect,
        color: instance.color,
        params: [instance.transform2[0], 0.0, 0.0, 0.0],
        clip: [
            instance.rect[0] + instance.clip[0],
            instance.rect[1] + instance.clip[1],
            instance.rect[0] + instance.clip[2],
            instance.rect[1] + instance.clip[3],
        ],
    }
}

fn should_collapse_split_batches(rect_count: usize, batch_count: usize, line_count: usize) -> bool {
    if line_count > 0 && line_count * 2 >= rect_count {
        return false;
    }
    batch_count > 512 && batch_count > (rect_count / 4).max(64)
}

// ---------------------------------------------------------------------------
// Widget-tree to RectInstance mapping
// ---------------------------------------------------------------------------

fn inst(rect: [f32; 4], color: [f32; 4], radius: f32) -> RectInstance {
    inst_radii(rect, color, [radius; 4])
}

fn inst_radii(rect: [f32; 4], color: [f32; 4], radii: [f32; 4]) -> RectInstance {
    inst_radii_clipped(
        rect,
        color,
        radii,
        [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
    )
}

fn inst_radii_clipped(
    rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    clip: [f32; 4],
) -> RectInstance {
    RectInstance {
        rect,
        color,
        radii,
        clip,
        params: [1.0, 0.0, 0.0, 0.0],
        color2: color,
        paint: [0.0, 0.0, 0.0, 0.0],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: color,
        color4: color,
        gradient_stops: [0.0, 1.0, 1.0, 1.0],
        color5: color,
        color6: color,
        gradient_stops2: [1.0, 1.0, 1.0, 1.0],
    }
}

fn inst_rounded_triangle(rect: [f32; 4], color: [f32; 4], up: bool, radius: f32) -> RectInstance {
    inst_rounded_triangle_clipped(
        rect,
        color,
        up,
        radius,
        [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
    )
}

fn inst_rounded_triangle_clipped(
    rect: [f32; 4],
    color: [f32; 4],
    up: bool,
    radius: f32,
    clip: [f32; 4],
) -> RectInstance {
    RectInstance {
        rect,
        color,
        radii: [radius; 4],
        clip,
        params: [1.0, 0.0, 0.0, 1.0],
        color2: color,
        paint: [0.0, 0.0, 0.0, if up { 1.0 } else { 0.0 }],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: color,
        color4: color,
        gradient_stops: [0.0, 1.0, 1.0, 1.0],
        color5: color,
        color6: color,
        gradient_stops2: [1.0, 1.0, 1.0, 1.0],
    }
}

fn inst_pie_slice(
    rect: [f32; 4],
    color: [f32; 4],
    start_rad: f32,
    end_rad: f32,
    inner_ratio: f32,
) -> RectInstance {
    let mut instance = inst_radii(rect, color, [0.0; 4]);
    instance.params = [1.0, 0.0, 0.0, 2.0];
    instance.paint = [0.0, start_rad, end_rad, inner_ratio.clamp(0.0, 0.9)];
    instance
}

fn inst_loading_spinner(
    rect: [f32; 4],
    track_color: [f32; 4],
    arc_color: [f32; 4],
    phase_rad: f32,
    sweep_rad: f32,
    inner_ratio: f32,
    tail_alpha: f32,
) -> RectInstance {
    let mut instance = inst_radii(rect, arc_color, [0.0; 4]);
    instance.params = [1.0, 0.0, 0.0, 4.0];
    instance.color2 = track_color;
    instance.paint = [
        0.0,
        phase_rad,
        sweep_rad.clamp(0.001, LOADING_SPINNER_TAU),
        inner_ratio.clamp(0.0, 0.95),
    ];
    instance.gradient_stops[0] = tail_alpha.clamp(0.0, 1.0);
    instance
}

fn inst_progress_fill(
    rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    progress_width: f32,
) -> RectInstance {
    let mut instance = inst_radii(rect, color, radii);
    instance.params[3] = 5.0;
    instance.paint[3] = progress_width.max(0.0).min(rect[2]);
    instance
}

fn inst_outline_ring_clipped(
    rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    thickness: f32,
    clip: [f32; 4],
) -> RectInstance {
    RectInstance {
        rect,
        color,
        radii,
        clip,
        params: [1.0, 0.0, 3.0, 0.0],
        color2: color,
        paint: [0.0, 0.0, 0.0, thickness.max(0.0)],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: color,
        color4: color,
        gradient_stops: [0.0, 1.0, 1.0, 1.0],
        color5: color,
        color6: color,
        gradient_stops2: [1.0, 1.0, 1.0, 1.0],
    }
}

fn border_pattern_code(style: BorderLineStyle) -> f32 {
    match style {
        BorderLineStyle::None | BorderLineStyle::Solid => 0.0,
        BorderLineStyle::Dotted => 10.0,
        BorderLineStyle::Dashed => 11.0,
        BorderLineStyle::Double => 12.0,
    }
}

fn inst_patterned_outline_ring_clipped(
    rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    thickness: f32,
    clip: [f32; 4],
    style: BorderLineStyle,
) -> RectInstance {
    let mut instance = inst_outline_ring_clipped(rect, color, radii, thickness, clip);
    instance.paint[0] = border_pattern_code(style);
    instance
}

fn inst_patterned_border_strip(
    rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    horizontal: bool,
    style: BorderLineStyle,
) -> RectInstance {
    let mut instance = inst_radii(rect, color, radii);
    instance.params[3] = 6.0;
    instance.paint[0] = border_pattern_code(style);
    instance.paint[1] = if horizontal { 1.0 } else { 0.0 };
    instance
}

fn default_local_clip(rect: [f32; 4]) -> [f32; 4] {
    [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0]
}

fn local_clip_for_rect(rect: [f32; 4], clip: Option<Rect>) -> Option<[f32; 4]> {
    local_clip_for_translated_rect(rect, [0.0, 0.0], clip)
}

fn local_clip_for_translated_rect(
    rect: [f32; 4],
    translate: [f32; 2],
    clip: Option<Rect>,
) -> Option<[f32; 4]> {
    let Some(clip) = clip else {
        return Some(default_local_clip(rect));
    };
    let aa_pad = 1.0;
    let clip = Rect {
        x: clip.x - aa_pad,
        y: clip.y - aa_pad,
        w: clip.w + aa_pad * 2.0,
        h: clip.h + aa_pad * 2.0,
    };
    let visible = Rect {
        x: rect[0] + translate[0] - aa_pad,
        y: rect[1] + translate[1] - aa_pad,
        w: rect[2] + aa_pad * 2.0,
        h: rect[3] + aa_pad * 2.0,
    }
    .intersect(clip)?;
    Some([
        visible.x - (rect[0] + translate[0]),
        visible.y - (rect[1] + translate[1]),
        visible.x + visible.w - (rect[0] + translate[0]),
        visible.y + visible.h - (rect[1] + translate[1]),
    ])
}

fn intersect_local_clip(current: [f32; 4], next: [f32; 4]) -> Option<[f32; 4]> {
    let clip = [
        current[0].max(next[0]),
        current[1].max(next[1]),
        current[2].min(next[2]),
        current[3].min(next[3]),
    ];
    (clip[2] > clip[0] && clip[3] > clip[1]).then_some(clip)
}

fn apply_paint_clip(instances: &mut [RectInstance], clip: Option<Rect>) {
    let Some(clip) = clip else {
        return;
    };
    for inst in instances {
        let next = local_clip_for_translated_rect(
            inst.rect,
            [inst.transform[0], inst.transform[1]],
            Some(clip),
        );
        inst.clip = next
            .and_then(|next| intersect_local_clip(inst.clip, next))
            .unwrap_or([1.0, 1.0, 0.0, 0.0]);
    }
}

fn inst_shadow_clipped(
    rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    blur: f32,
    clip: [f32; 4],
) -> RectInstance {
    RectInstance {
        rect,
        color,
        radii,
        clip,
        params: [blur.max(1.0), blur.max(0.0), 1.0, 0.0],
        color2: color,
        paint: [0.0, 0.0, 0.0, 0.0],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: color,
        color4: color,
        gradient_stops: [0.0, 1.0, 1.0, 1.0],
        color5: color,
        color6: color,
        gradient_stops2: [1.0, 1.0, 1.0, 1.0],
    }
}

fn inst_inset_shadow(
    rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    blur: f32,
    offset: [f32; 2],
    spread: f32,
) -> RectInstance {
    RectInstance {
        rect,
        color,
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [blur.max(1.0), 0.0, 2.0, 0.0],
        color2: color,
        paint: [0.0, offset[0], offset[1], spread],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: color,
        color4: color,
        gradient_stops: [0.0, 1.0, 1.0, 1.0],
        color5: color,
        color6: color,
        gradient_stops2: [1.0, 1.0, 1.0, 1.0],
    }
}

fn inst_linear_gradient(
    rect: [f32; 4],
    colors: [[f32; 4]; GRADIENT_STOP_CAPACITY],
    stops: [f32; GRADIENT_STOP_CAPACITY],
    count: f32,
    interpolation: f32,
    radii: [f32; 4],
    angle_deg: f32,
) -> RectInstance {
    let angle = angle_deg.to_radians();
    let dir = [angle.sin(), -angle.cos()];
    RectInstance {
        rect,
        color: colors[0],
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [1.0, 0.0, 0.0, 0.0],
        color2: colors[1],
        paint: [1.0, dir[0], dir[1], count],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, interpolation, 0.0],
        color3: colors[2],
        color4: colors[3],
        gradient_stops: [stops[0], stops[1], stops[2], stops[3]],
        color5: colors[4],
        color6: colors[5],
        gradient_stops2: [stops[4], stops[5], 1.0, 1.0],
    }
}

fn inst_radial_gradient(
    rect: [f32; 4],
    colors: [[f32; 4]; GRADIENT_STOP_CAPACITY],
    stops: [f32; GRADIENT_STOP_CAPACITY],
    count: f32,
    interpolation: f32,
    radii: [f32; 4],
    center: [f32; 2],
) -> RectInstance {
    RectInstance {
        rect,
        color: colors[0],
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [1.0, 0.0, 0.0, 0.0],
        color2: colors[1],
        paint: [2.0, center[0], center[1], count],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, interpolation, 0.0],
        color3: colors[2],
        color4: colors[3],
        gradient_stops: [stops[0], stops[1], stops[2], stops[3]],
        color5: colors[4],
        color6: colors[5],
        gradient_stops2: [stops[4], stops[5], 1.0, 1.0],
    }
}

fn inst_blob_gradient(
    rect: [f32; 4],
    colors: [[f32; 4]; 4],
    centers: [[f32; 2]; 4],
    radii_values: [f32; 4],
    count: f32,
    interpolation: f32,
    radii: [f32; 4],
) -> RectInstance {
    RectInstance {
        rect,
        color: colors[0],
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [1.0, 0.0, 0.0, 0.0],
        color2: colors[1],
        paint: [3.0, 0.0, 0.0, count],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, interpolation, 0.0],
        color3: colors[2],
        color4: colors[3],
        gradient_stops: [centers[0][0], centers[0][1], centers[1][0], centers[1][1]],
        color5: [
            radii_values[0],
            radii_values[1],
            radii_values[2],
            radii_values[3],
        ],
        color6: [0.0, 0.0, 0.0, 0.0],
        gradient_stops2: [centers[2][0], centers[2][1], centers[3][0], centers[3][1]],
    }
}

fn inst_mesh_gradient(
    rect: [f32; 4],
    colors: [[f32; 4]; 4],
    interpolation: f32,
    radii: [f32; 4],
) -> RectInstance {
    RectInstance {
        rect,
        color: colors[0],
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [1.0, 0.0, 0.0, 0.0],
        color2: colors[1],
        paint: [4.0, 0.0, 0.0, 4.0],
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, interpolation, 0.0],
        color3: colors[2],
        color4: colors[3],
        gradient_stops: [0.0, 1.0, 0.0, 1.0],
        color5: [0.0, 0.0, 0.0, 0.0],
        color6: [0.0, 0.0, 0.0, 0.0],
        gradient_stops2: [1.0, 1.0, 1.0, 1.0],
    }
}

fn background_pattern_kind_code(kind: BackgroundPatternKind) -> f32 {
    match kind {
        BackgroundPatternKind::Checker => 0.0,
        BackgroundPatternKind::Pinstripe => 1.0,
        BackgroundPatternKind::Dot => 2.0,
        BackgroundPatternKind::DiagonalHatch => 3.0,
    }
}

fn inst_background_pattern(
    rect: [f32; 4],
    foreground: [f32; 4],
    background: [f32; 4],
    kind: BackgroundPatternKind,
    tile_size_px: f32,
    radii: [f32; 4],
) -> RectInstance {
    let mut instance = inst_radii(rect, foreground, radii);
    instance.color2 = background;
    instance.paint = [
        5.0,
        background_pattern_kind_code(kind),
        tile_size_px.max(1.0),
        0.0,
    ];
    instance
}

fn push_masked_rect(
    out: &mut Vec<RectInstance>,
    mask_rect: [f32; 4],
    color: [f32; 4],
    radii: [f32; 4],
    rect: [f32; 4],
) {
    if rect[2] <= 0.0 || rect[3] <= 0.0 || mask_rect[2] <= 0.0 || mask_rect[3] <= 0.0 {
        return;
    }
    let clip = [
        rect[0] - mask_rect[0],
        rect[1] - mask_rect[1],
        rect[0] + rect[2] - mask_rect[0],
        rect[1] + rect[3] - mask_rect[1],
    ];
    out.push(inst_radii_clipped(mask_rect, color, radii, clip));
}

fn apply_transform_to_instances(
    instances: &mut [RectInstance],
    transform: Option<TransformStyle>,
    sf: f32,
    origin: [f32; 2],
) {
    let Some(transform) = transform.filter(|transform| !transform.is_identity()) else {
        return;
    };
    let parent_translate = [transform.translate_x * sf, transform.translate_y * sf];
    let parent_scale = [transform.scale_x, transform.scale_y];
    let rotation = transform.rotate_deg.to_radians();
    let cos_r = rotation.cos();
    let sin_r = rotation.sin();
    for instance in instances {
        let center = [
            instance.rect[0] + instance.rect[2] * 0.5,
            instance.rect[1] + instance.rect[3] * 0.5,
        ];
        let current_center = [
            center[0] + instance.transform[0],
            center[1] + instance.transform[1],
        ];
        let scaled = [
            (current_center[0] - origin[0]) * parent_scale[0],
            (current_center[1] - origin[1]) * parent_scale[1],
        ];
        let rotated = [
            scaled[0] * cos_r - scaled[1] * sin_r,
            scaled[0] * sin_r + scaled[1] * cos_r,
        ];
        let transformed_center = [
            origin[0] + rotated[0] + parent_translate[0],
            origin[1] + rotated[1] + parent_translate[1],
        ];
        instance.transform[0] = transformed_center[0] - center[0];
        instance.transform[1] = transformed_center[1] - center[1];
        instance.transform[2] *= parent_scale[0];
        instance.transform[3] *= parent_scale[1];
        instance.transform2[0] += rotation;
    }
}

fn apply_background_noise_to_instances(instances: &mut [RectInstance], noise: Option<f32>) {
    let Some(noise) = noise
        .map(|value| value.clamp(0.0, 0.25))
        .filter(|value| *value > 0.0)
    else {
        return;
    };
    for instance in instances {
        if instance.params[2] < 0.5 && instance.paint[0] > 0.5 {
            instance.transform2[1] = noise;
        }
    }
}

fn backdrop_filter_noise(visual: &VisualStyle) -> Option<f32> {
    visual
        .backdrop_filter
        .map(|filter| (filter.blur / 720.0).clamp(0.0, 0.045))
        .filter(|noise| *noise > 0.0)
}

fn widget_supports_backdrop_filter(kind: WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::Panel | WidgetKind::Modal | WidgetKind::Tooltip | WidgetKind::Toast
    )
}

fn emit_backdrop_filter_tint(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    radii: [f32; 4],
    filter: BackdropFilterStyle,
) {
    if filter.is_identity() {
        return;
    }
    let blur_alpha = if filter.blur > 0.0 {
        (filter.blur / 180.0).clamp(0.025, 0.095)
    } else {
        0.0
    };
    let brightness_delta = (filter.brightness - 1.0).clamp(-1.0, 1.0);
    let saturate_delta = (filter.saturate - 1.0).abs().min(2.0);
    let alpha =
        (blur_alpha + brightness_delta.abs() * 0.10 + saturate_delta * 0.025).clamp(0.015, 0.16);
    let color = if brightness_delta < -0.001 {
        [0.0, 0.0, 0.0, alpha]
    } else if filter.saturate > 1.0 {
        [0.92, 0.97, 1.0, alpha]
    } else {
        [1.0, 1.0, 1.0, alpha]
    };
    out.push(inst_radii(rect, color, radii));
}

fn paint_transform_for_node(
    node: &WidgetNode,
    visual_transform: Option<TransformStyle>,
) -> Option<TransformStyle> {
    let mut transform = visual_transform.unwrap_or_default();
    if node.style.layout.position == Some(PositionStyle::Relative) {
        transform.translate_x += node.style.layout.left.unwrap_or(0.0);
        transform.translate_x -= node.style.layout.right.unwrap_or(0.0);
        transform.translate_y += node.style.layout.top.unwrap_or(0.0);
        transform.translate_y -= node.style.layout.bottom.unwrap_or(0.0);
    }
    (!transform.is_identity()).then_some(transform)
}

fn visit_stacking_children<'a>(node: &'a WidgetNode, mut visit: impl FnMut(&'a WidgetNode)) {
    if node.children.len() <= 1
        || node
            .children
            .iter()
            .all(|child| child.style.layout.z_index.unwrap_or(0) == 0)
    {
        for child in &node.children {
            visit(child);
        }
        return;
    }

    let mut children: Vec<_> = node.children.iter().enumerate().collect();
    children.sort_by_key(|(index, child)| (child.style.layout.z_index.unwrap_or(0), *index));
    for (_, child) in children {
        visit(child);
    }
}

fn inset_radii(radii: [f32; 4], inset: f32) -> [f32; 4] {
    radii.map(|radius| (radius - inset).max(0.0))
}

fn outset_radii(radii: [f32; 4], outset: f32) -> [f32; 4] {
    radii.map(|radius| {
        if radius <= 0.0 {
            0.0
        } else {
            (radius + outset).max(0.0)
        }
    })
}

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn mix_premultiplied_alpha(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    let left_alpha = a[3].clamp(0.0, 1.0);
    let right_alpha = b[3].clamp(0.0, 1.0);
    let alpha = left_alpha + (right_alpha - left_alpha) * t;
    if alpha <= 0.0001 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let left = [a[0] * left_alpha, a[1] * left_alpha, a[2] * left_alpha];
    let right = [b[0] * right_alpha, b[1] * right_alpha, b[2] * right_alpha];
    [
        (left[0] + (right[0] - left[0]) * t) / alpha,
        (left[1] + (right[1] - left[1]) * t) / alpha,
        (left[2] + (right[2] - left[2]) * t) / alpha,
        alpha,
    ]
}

fn with_alpha(mut color: [f32; 4], alpha: f32) -> [f32; 4] {
    color[3] = alpha.clamp(0.0, 1.0);
    color
}

fn darken(color: [f32; 4], t: f32) -> [f32; 4] {
    mix(color, [0.0, 0.0, 0.0, color[3]], t)
}

pub(crate) fn visual_for<'a>(
    node: &'a WidgetNode,
    state: &WidgetState,
    theme: &Theme,
) -> Cow<'a, VisualStyle> {
    let base = &node.style.visual;
    let mut visual = base.clone();
    let mut changed = false;
    if let Some(t) = state.checked_t.get(&node.id).copied() {
        let base_state = visual.clone();
        let checked = visual.merged(&node.style.checked);
        let current_state = if state.checked.get(&node.id).copied().unwrap_or(false) {
            &checked
        } else {
            &base_state
        };
        visual = interpolate_visual_style(
            &base_state,
            &checked,
            current_state,
            t,
            theme,
            node.style.transition.properties.as_deref(),
        );
        changed = true;
    } else {
        merge_checked_visual_state(&mut visual, node, state, &mut changed);
    }
    if let Some(t) = state.open_t.get(&node.id).copied() {
        let base_state = visual.clone();
        let open = visual.merged(&node.style.open);
        let current_state = if node_is_open(node, state) {
            &open
        } else {
            &base_state
        };
        visual = interpolate_visual_style(
            &base_state,
            &open,
            current_state,
            t,
            theme,
            node.style.transition.properties.as_deref(),
        );
        changed = true;
    } else if node_is_open(node, state) {
        visual = visual.merged(&node.style.open);
        changed = true;
    }
    if let Some(t) = state.expanded_t.get(&node.id).copied() {
        let base_state = visual.clone();
        let expanded = visual.merged(&node.style.expanded);
        let collapsed = visual.merged(&node.style.collapsed);
        let current_state = if state.is_expanded_widget(&node.id) {
            &expanded
        } else if state.is_collapsed_widget(&node.id) {
            &collapsed
        } else {
            &base_state
        };
        visual = interpolate_visual_style(
            &collapsed,
            &expanded,
            current_state,
            t,
            theme,
            node.style.transition.properties.as_deref(),
        );
        changed = true;
    } else {
        merge_expansion_visual_states(&mut visual, node, state, &mut changed);
    }
    if let Some(t) = state.selected_t.get(&node.id).copied() {
        let base_state = visual.clone();
        let selected = visual.merged(&node.style.selected);
        let current_state = if state.is_selected_widget(&node.id) {
            &selected
        } else {
            &base_state
        };
        visual = interpolate_visual_style(
            &base_state,
            &selected,
            current_state,
            t,
            theme,
            node.style.transition.properties.as_deref(),
        );
        changed = true;
    } else if state.is_selected_widget(&node.id) {
        visual = visual.merged(&node.style.selected);
        changed = true;
    }
    if let Some(t) = state.hover_t.get(&node.id).copied() {
        let base_state = visual.clone();
        let hover = visual.merged(&node.style.hover);
        let current_state = if state.hovered.as_deref() == Some(node.id.as_str()) {
            &hover
        } else {
            &base_state
        };
        visual = interpolate_visual_style(
            &base_state,
            &hover,
            current_state,
            t,
            theme,
            node.style.transition.properties.as_deref(),
        );
        changed = true;
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        visual = visual.merged(&node.style.hover);
        changed = true;
    }
    if let Some(t) = state.active_t.get(&node.id).copied() {
        let base_state = visual.clone();
        let active = visual.merged(&node.style.active);
        let current_state = if state.pressed.as_deref() == Some(node.id.as_str()) {
            &active
        } else {
            &base_state
        };
        visual = interpolate_visual_style(
            &base_state,
            &active,
            current_state,
            t,
            theme,
            node.style.transition.properties.as_deref(),
        );
        changed = true;
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        visual = visual.merged(&node.style.active);
        changed = true;
    } else if let Some(t) = state.focus_t.get(&node.id).copied() {
        let base_state = visual.clone();
        let focus = visual.merged(&node.style.focus);
        let current_state = if state.focused.as_deref() == Some(node.id.as_str()) {
            &focus
        } else {
            &base_state
        };
        visual = interpolate_visual_style(
            &base_state,
            &focus,
            current_state,
            t,
            theme,
            node.style.transition.properties.as_deref(),
        );
        changed = true;
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        visual = visual.merged(&node.style.focus);
        changed = true;
    }
    if state.is_disabled(&node.id) {
        visual = visual.merged(&node.style.disabled);
        changed = true;
    }
    if let Some(animation) = state.animation_visuals.get(&node.id) {
        visual = visual.merged(animation);
        changed = true;
    }
    if changed {
        Cow::Owned(visual)
    } else {
        Cow::Borrowed(base)
    }
}

pub(crate) fn interpolate_visual_style(
    from: &VisualStyle,
    to: &VisualStyle,
    instant: &VisualStyle,
    t: f32,
    theme: &Theme,
    properties: Option<&[TransitionProperty]>,
) -> VisualStyle {
    let t = t.clamp(0.0, 1.0);
    VisualStyle {
        background: if transition_allows(properties, TransitionProperty::Background) {
            interpolate_color_ref(&from.background, &to.background, t, theme)
        } else {
            instant.background.clone()
        },
        background_paint: if transition_allows(properties, TransitionProperty::Background) {
            interpolate_background_paint(&from.background_paint, &to.background_paint, t, theme)
        } else {
            instant.background_paint.clone()
        },
        gradient_interpolation: instant.gradient_interpolation,
        backdrop_filter: instant.backdrop_filter,
        foreground: if transition_allows_any(
            properties,
            &[TransitionProperty::Foreground, TransitionProperty::Color],
        ) {
            interpolate_color_ref(&from.foreground, &to.foreground, t, theme)
        } else {
            instant.foreground.clone()
        },
        border_color: if transition_allows(properties, TransitionProperty::BorderColor) {
            interpolate_color_ref(&from.border_color, &to.border_color, t, theme)
        } else {
            instant.border_color.clone()
        },
        border_width: if transition_allows(properties, TransitionProperty::BorderWidth) {
            interpolate_option_f32(from.border_width, to.border_width, t)
        } else {
            instant.border_width
        },
        border_style: instant.border_style,
        border_top_color: if transition_allows(properties, TransitionProperty::BorderColor) {
            interpolate_color_ref(&from.border_top_color, &to.border_top_color, t, theme)
        } else {
            instant.border_top_color.clone()
        },
        border_right_color: if transition_allows(properties, TransitionProperty::BorderColor) {
            interpolate_color_ref(&from.border_right_color, &to.border_right_color, t, theme)
        } else {
            instant.border_right_color.clone()
        },
        border_bottom_color: if transition_allows(properties, TransitionProperty::BorderColor) {
            interpolate_color_ref(&from.border_bottom_color, &to.border_bottom_color, t, theme)
        } else {
            instant.border_bottom_color.clone()
        },
        border_left_color: if transition_allows(properties, TransitionProperty::BorderColor) {
            interpolate_color_ref(&from.border_left_color, &to.border_left_color, t, theme)
        } else {
            instant.border_left_color.clone()
        },
        border_top_width: if transition_allows(properties, TransitionProperty::BorderWidth) {
            interpolate_option_f32(from.border_top_width, to.border_top_width, t)
        } else {
            instant.border_top_width
        },
        border_right_width: if transition_allows(properties, TransitionProperty::BorderWidth) {
            interpolate_option_f32(from.border_right_width, to.border_right_width, t)
        } else {
            instant.border_right_width
        },
        border_bottom_width: if transition_allows(properties, TransitionProperty::BorderWidth) {
            interpolate_option_f32(from.border_bottom_width, to.border_bottom_width, t)
        } else {
            instant.border_bottom_width
        },
        border_left_width: if transition_allows(properties, TransitionProperty::BorderWidth) {
            interpolate_option_f32(from.border_left_width, to.border_left_width, t)
        } else {
            instant.border_left_width
        },
        border_top_style: instant.border_top_style,
        border_right_style: instant.border_right_style,
        border_bottom_style: instant.border_bottom_style,
        border_left_style: instant.border_left_style,
        outline_color: if transition_allows_any(
            properties,
            &[
                TransitionProperty::Outline,
                TransitionProperty::OutlineColor,
            ],
        ) {
            interpolate_color_ref(&from.outline_color, &to.outline_color, t, theme)
        } else {
            instant.outline_color.clone()
        },
        outline_width: if transition_allows_any(
            properties,
            &[
                TransitionProperty::Outline,
                TransitionProperty::OutlineWidth,
            ],
        ) {
            interpolate_option_f32(from.outline_width, to.outline_width, t)
        } else {
            instant.outline_width
        },
        outline_style: instant.outline_style,
        outline_offset: if transition_allows_any(
            properties,
            &[
                TransitionProperty::Outline,
                TransitionProperty::OutlineOffset,
            ],
        ) {
            interpolate_option_f32(from.outline_offset, to.outline_offset, t)
        } else {
            instant.outline_offset
        },
        border_radius: if transition_allows(properties, TransitionProperty::BorderRadius) {
            interpolate_option_f32(from.border_radius, to.border_radius, t)
        } else {
            instant.border_radius
        },
        corner_radii: if transition_allows(properties, TransitionProperty::BorderRadius) {
            crate::style::CornerRadii {
                top_left: interpolate_option_f32(
                    from.corner_radii.top_left,
                    to.corner_radii.top_left,
                    t,
                ),
                top_right: interpolate_option_f32(
                    from.corner_radii.top_right,
                    to.corner_radii.top_right,
                    t,
                ),
                bottom_right: interpolate_option_f32(
                    from.corner_radii.bottom_right,
                    to.corner_radii.bottom_right,
                    t,
                ),
                bottom_left: interpolate_option_f32(
                    from.corner_radii.bottom_left,
                    to.corner_radii.bottom_left,
                    t,
                ),
            }
        } else {
            instant.corner_radii
        },
        accent: if transition_allows(properties, TransitionProperty::Accent) {
            interpolate_color_ref(&from.accent, &to.accent, t, theme)
        } else {
            instant.accent.clone()
        },
        track_color: if transition_allows(properties, TransitionProperty::TrackColor) {
            interpolate_color_ref(&from.track_color, &to.track_color, t, theme)
        } else {
            instant.track_color.clone()
        },
        thumb_color: if transition_allows(properties, TransitionProperty::ThumbColor) {
            interpolate_color_ref(&from.thumb_color, &to.thumb_color, t, theme)
        } else {
            instant.thumb_color.clone()
        },
        opacity: if transition_allows(properties, TransitionProperty::Opacity) {
            interpolate_option_f32(from.opacity, to.opacity, t)
        } else {
            instant.opacity
        },
        background_noise: if transition_allows(properties, TransitionProperty::Background) {
            interpolate_option_f32(from.background_noise, to.background_noise, t)
        } else {
            instant.background_noise
        },
        box_shadows: if transition_allows(properties, TransitionProperty::BoxShadow) {
            if t < 0.5 {
                from.box_shadows.clone()
            } else {
                to.box_shadows.clone()
            }
        } else {
            instant.box_shadows.clone()
        },
        transform: if transition_allows(properties, TransitionProperty::Transform) {
            interpolate_transform(from.transform, to.transform, t)
        } else {
            instant.transform
        },
    }
}

fn transition_allows(
    properties: Option<&[TransitionProperty]>,
    property: TransitionProperty,
) -> bool {
    properties.is_none_or(|properties| {
        properties.contains(&TransitionProperty::All) || properties.contains(&property)
    })
}

fn transition_allows_any(
    properties: Option<&[TransitionProperty]>,
    candidates: &[TransitionProperty],
) -> bool {
    properties.is_none_or(|properties| {
        properties.contains(&TransitionProperty::All)
            || candidates
                .iter()
                .any(|candidate| properties.contains(candidate))
    })
}

fn interpolate_background_paint(
    from: &Option<BackgroundPaint>,
    to: &Option<BackgroundPaint>,
    t: f32,
    theme: &Theme,
) -> Option<BackgroundPaint> {
    match (from, to) {
        (Some(BackgroundPaint::Color(a)), Some(BackgroundPaint::Color(b))) => Some(
            BackgroundPaint::Color(ColorRef::Rgba(mix(a.resolve(theme), b.resolve(theme), t))),
        ),
        _ if t < 0.5 => from.clone(),
        _ => to.clone(),
    }
}

fn interpolate_color_ref(
    from: &Option<ColorRef>,
    to: &Option<ColorRef>,
    t: f32,
    theme: &Theme,
) -> Option<ColorRef> {
    match (from, to) {
        (Some(a), Some(b)) => Some(ColorRef::Rgba(mix(a.resolve(theme), b.resolve(theme), t))),
        _ if t < 0.5 => from.clone(),
        _ => to.clone(),
    }
}

fn interpolate_option_f32(from: Option<f32>, to: Option<f32>, t: f32) -> Option<f32> {
    match (from, to) {
        (Some(a), Some(b)) => Some(a + (b - a) * t),
        _ if t < 0.5 => from,
        _ => to,
    }
}

fn interpolate_transform(
    from: Option<TransformStyle>,
    to: Option<TransformStyle>,
    t: f32,
) -> Option<TransformStyle> {
    let from = from.unwrap_or_default();
    let to = to.unwrap_or_default();
    let transform = TransformStyle {
        translate_x: from.translate_x + (to.translate_x - from.translate_x) * t,
        translate_y: from.translate_y + (to.translate_y - from.translate_y) * t,
        scale_x: from.scale_x + (to.scale_x - from.scale_x) * t,
        scale_y: from.scale_y + (to.scale_y - from.scale_y) * t,
        rotate_deg: from.rotate_deg + (to.rotate_deg - from.rotate_deg) * t,
    };
    (!transform.is_identity()).then_some(transform)
}

fn merge_checked_visual_state(
    visual: &mut VisualStyle,
    node: &WidgetNode,
    state: &WidgetState,
    changed: &mut bool,
) {
    if state.checked.get(&node.id).copied().unwrap_or(false) {
        *visual = visual.merged(&node.style.checked);
        *changed = true;
    }
}

fn merge_expansion_visual_states(
    visual: &mut VisualStyle,
    node: &WidgetNode,
    state: &WidgetState,
    changed: &mut bool,
) {
    if state.is_expanded_widget(&node.id) {
        *visual = visual.merged(&node.style.expanded);
        *changed = true;
    }
    if state.is_collapsed_widget(&node.id) {
        *visual = visual.merged(&node.style.collapsed);
        *changed = true;
    }
}

fn node_is_open(node: &WidgetNode, state: &WidgetState) -> bool {
    state.is_open_widget(&node.id)
        || (node.kind == WidgetKind::Modal && node.props.open == Some(true))
}

fn part_visual_for(node: &WidgetNode, state: &WidgetState, part: &str) -> VisualStyle {
    style_part_visual_for_state(&node.style, &node.id, state, part)
}

fn part_style_active_for_state(node: &WidgetNode, state: &WidgetState, part: &str) -> bool {
    style_part_style_active_for_state(&node.style, &node.id, state, part)
}

fn merged_part_visual_for(node: &WidgetNode, state: &WidgetState, parts: &[&str]) -> VisualStyle {
    style_merged_part_visual_for_state(&node.style, &node.id, state, parts)
}

fn emit_titled_container_surface_parts(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    container_radii: [f32; 4],
    container_border_w: f32,
) {
    let Some(geometry) = titled_container_geometry(node, layout, sf, theme) else {
        return;
    };
    let inner_radii = inset_radii(container_radii, container_border_w);
    for (part, rect, inherited_radii) in [
        (
            "header",
            geometry.title_band,
            [inner_radii[0], inner_radii[1], 0.0, 0.0],
        ),
        (
            "body",
            geometry.body_viewport,
            [0.0, 0.0, inner_radii[2], inner_radii[3]],
        ),
    ] {
        if !part_style_active_for_state(node, state, part) || rect.w <= 0.0 || rect.h <= 0.0 {
            continue;
        }
        let part_visual = part_visual_for(node, state, part);
        let part_border_w = part_visual.border_width.unwrap_or(0.0).max(0.0) * sf;
        let part_radii = part_visual
            .border_radius
            .map(|radius| visual_radii(&part_visual, radius.max(0.0), sf))
            .unwrap_or(inherited_radii);
        let part_fill = resolve_background_paint(&part_visual, theme, [0.0, 0.0, 0.0, 0.0], sf);
        let part_border = resolve_color(&part_visual.border_color, theme)
            .map(|color| apply_opacity(color, part_visual.opacity))
            .unwrap_or(theme.border);
        emit_bordered_paint_rect_radii(
            out,
            [rect.x, rect.y, rect.w, rect.h],
            part_border,
            part_fill,
            part_radii,
            part_border_w,
        );
    }
}

fn resolve_color(color: &Option<crate::style::ColorRef>, theme: &Theme) -> Option<[f32; 4]> {
    color.as_ref().map(|c| c.resolve(theme))
}

#[derive(Debug, Clone)]
enum FillPaint {
    Solid([f32; 4]),
    Layers(Vec<FillPaint>),
    LinearGradient {
        stops: Vec<ResolvedGradientStop>,
        repeating: bool,
        scale_factor: f32,
        interpolation: f32,
        angle_deg: f32,
    },
    RadialGradient {
        stops: Vec<ResolvedGradientStop>,
        repeating: bool,
        scale_factor: f32,
        interpolation: f32,
        center: [f32; 2],
    },
    BlobGradient {
        colors: [[f32; 4]; 4],
        centers: [[f32; 2]; 4],
        radii: [f32; 4],
        count: f32,
        interpolation: f32,
    },
    MeshGradient {
        colors: [[f32; 4]; 4],
        interpolation: f32,
    },
    Pattern {
        kind: BackgroundPatternKind,
        foreground: [f32; 4],
        background: [f32; 4],
        tile_size_px: f32,
    },
}

#[derive(Debug, Clone)]
struct ResolvedGradientStop {
    color: [f32; 4],
    position: Option<crate::style::CalcLength>,
}

const GRADIENT_STOP_CAPACITY: usize = 6;

fn apply_opacity(mut color: [f32; 4], opacity: Option<f32>) -> [f32; 4] {
    if let Some(opacity) = opacity {
        color[3] *= opacity.clamp(0.0, 1.0);
    }
    color
}

const LINE_PLOT_MAX_SEGMENTS_PER_SERIES: usize = 4096;
const LINE_PLOT_JOIN_DOT_COS_THRESHOLD: f32 = 0.985;
const LINE_PLOT_AXIS_LABEL_FONT_SIZE_LP: f32 = 14.0;
const LINE_PLOT_AXIS_LABEL_GUTTER_LP: f32 = 18.0;
const LINE_PLOT_PALETTE: [[f32; 4]; 6] = [
    [0.33, 0.66, 1.00, 1.0],
    [0.30, 0.84, 0.52, 1.0],
    [1.00, 0.65, 0.22, 1.0],
    [0.94, 0.39, 0.48, 1.0],
    [0.72, 0.56, 1.00, 1.0],
    [0.26, 0.80, 0.82, 1.0],
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct LinePlotBounds {
    pub x_min: f32,
    pub x_max: f32,
    pub y_min: f32,
    pub y_max: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct LinePlotTextLabel {
    pub text: String,
    pub screen_x: f32,
    pub screen_y: f32,
    pub is_title: bool,
    pub anchor: &'static str,
    pub color: Option<[f32; 3]>,
    pub font_size: Option<f32>,
    pub clip_rect: Option<[f32; 4]>,
}

fn format_line_plot_hover_value(value: f32) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let abs = value.abs();
    if abs >= 10_000.0 || (abs > 0.0 && abs < 0.001) {
        format!("{value:.2e}")
    } else if abs >= 100.0 {
        format!("{value:.1}")
    } else if abs >= 10.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.3}")
    }
}

fn collect_line_plot_render_data(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    aa_width: f32,
    max_segments_per_series: usize,
    decimation_mode: LinePlotDecimationMode,
    inherited_clip: Option<Rect>,
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    build_stats: &mut LinePlotRenderBuildStats,
) {
    if node.kind == WidgetKind::Tooltip {
        return;
    }
    if node.kind == WidgetKind::Modal && !node.props.open.unwrap_or(false) {
        return;
    }

    let current_clip = match (inherited_clip, layout.paint_clip_rect(&node.id)) {
        (Some(a), Some(b)) => a.intersect(b),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    if node.kind == WidgetKind::LinePlot && layout.visible_rect(&node.id).is_some() {
        if let Some(rect) = layout.rects.get(&node.id).copied() {
            collect_line_plot_node_render_data(
                node,
                layout,
                theme,
                sf,
                rect,
                aa_width,
                max_segments_per_series,
                decimation_mode,
                current_clip,
                points,
                series_instances,
                build_stats,
            );
        }
    }

    visit_stacking_children(node, |child| {
        collect_line_plot_render_data(
            child,
            layout,
            theme,
            sf,
            aa_width,
            max_segments_per_series,
            decimation_mode,
            current_clip,
            points,
            series_instances,
            build_stats,
        );
    });
}

fn collect_line_plot_node_render_data(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    rect: Rect,
    aa_width: f32,
    max_segments_per_series: usize,
    decimation_mode: LinePlotDecimationMode,
    inherited_clip: Option<Rect>,
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    build_stats: &mut LinePlotRenderBuildStats,
) {
    if rect.w <= 2.0 || rect.h <= 2.0 {
        return;
    }
    let Some(bounds) = line_plot_resolved_bounds(node) else {
        return;
    };
    let plot = line_plot_plot_rect(node, sf, [rect.x, rect.y, rect.w, rect.h]);
    let mut clip = Rect {
        x: plot[0],
        y: plot[1],
        w: plot[2],
        h: plot[3],
    };
    if let Some(visible) = layout.visible_rect(&node.id) {
        let Some(next) = clip.intersect(visible) else {
            return;
        };
        clip = next;
    }
    if let Some(inherited_clip) = inherited_clip {
        let Some(next) = clip.intersect(inherited_clip) else {
            return;
        };
        clip = next;
    }
    if clip.w <= 1.0 || clip.h <= 1.0 {
        return;
    }

    let line_width = (node.props.line_plot_line_width.max(0.5) * sf)
        .max(1.0)
        .min(plot[3].max(1.0) * 0.10);
    let clip_rect = [clip.x, clip.y, clip.w, clip.h];
    for (series_index, series) in node.props.line_plot_series.iter().enumerate() {
        let Some(style_code) = line_plot_renderer_style_code(&series.line_style) else {
            continue;
        };
        let series_points = series.logical_points();
        if series_points.len() < 2 {
            continue;
        }
        let color = series
            .color
            .as_ref()
            .map(|color| color.resolve(theme))
            .unwrap_or(LINE_PLOT_PALETTE[series_index % LINE_PLOT_PALETTE.len()]);
        push_line_plot_renderer_series(
            points,
            series_instances,
            series_points,
            plot,
            clip_rect,
            bounds,
            line_width,
            color,
            aa_width,
            max_segments_per_series,
            decimation_mode,
            style_code,
            series.x_sorted,
            build_stats,
        );
    }
}

fn push_line_plot_renderer_series(
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    source: &[[f32; 2]],
    plot: [f32; 4],
    clip: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    aa_width: f32,
    max_segments_per_series: usize,
    decimation_mode: LinePlotDecimationMode,
    style_code: f32,
    x_sorted: bool,
    build_stats: &mut LinePlotRenderBuildStats,
) {
    let (start, end) = line_plot_visible_point_bounds(source, bounds);
    if end.saturating_sub(start) < 2 {
        return;
    }
    let visible = &source[start..end];
    build_stats.source_point_count += visible.len();
    let mut run_offset = points.len();
    let mut run_len = 0usize;
    let mut last_mapped = None;
    let mut path_distance = 0.0_f32;

    let decimate_t0 = Instant::now();
    let decimated = push_decimated_line_plot_renderer_series(
        points,
        series_instances,
        visible,
        plot,
        clip,
        bounds,
        line_width,
        color,
        aa_width,
        max_segments_per_series,
        decimation_mode,
        style_code,
        x_sorted,
        &mut run_offset,
        &mut run_len,
        &mut last_mapped,
        &mut path_distance,
    );
    if decimated {
        build_stats.decimated_series_count += 1;
        build_stats.decimate_ms += decimate_t0.elapsed().as_secs_f64() * 1000.0;
    } else {
        let segment_count = visible.len().saturating_sub(1).max(1);
        let stride =
            ((segment_count + max_segments_per_series - 1) / max_segments_per_series).max(1);
        let mut last_index = 0usize;
        for idx in (0..visible.len()).step_by(stride) {
            push_line_plot_renderer_point(
                points,
                series_instances,
                visible[idx],
                &mut run_offset,
                &mut run_len,
                plot,
                clip,
                bounds,
                line_width,
                color,
                aa_width,
                style_code,
                &mut last_mapped,
                &mut path_distance,
            );
            last_index = idx;
        }
        if last_index != visible.len() - 1 {
            push_line_plot_renderer_point(
                points,
                series_instances,
                visible[visible.len() - 1],
                &mut run_offset,
                &mut run_len,
                plot,
                clip,
                bounds,
                line_width,
                color,
                aa_width,
                style_code,
                &mut last_mapped,
                &mut path_distance,
            );
        }
    }
    flush_line_plot_renderer_run(
        points,
        series_instances,
        &mut run_offset,
        &mut run_len,
        plot,
        clip,
        bounds,
        line_width,
        color,
        aa_width,
        style_code,
    );
}

fn push_decimated_line_plot_renderer_series(
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    visible: &[[f32; 2]],
    plot: [f32; 4],
    clip: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    aa_width: f32,
    max_segments_per_series: usize,
    decimation_mode: LinePlotDecimationMode,
    style_code: f32,
    x_sorted: bool,
    run_offset: &mut usize,
    run_len: &mut usize,
    last_mapped: &mut Option<[f32; 2]>,
    path_distance: &mut f32,
) -> bool {
    if decimation_mode == LinePlotDecimationMode::Off || !x_sorted {
        return false;
    }
    if decimation_mode == LinePlotDecimationMode::Auto {
        return push_strided_line_plot_renderer_series(
            points,
            series_instances,
            visible,
            plot,
            clip,
            bounds,
            line_width,
            color,
            aa_width,
            max_segments_per_series,
            style_code,
            run_offset,
            run_len,
            last_mapped,
            path_distance,
        );
    }

    let bucket_count = line_plot_decimation_bucket_count(plot, max_segments_per_series);
    if bucket_count == 0 || visible.len() <= bucket_count.saturating_mul(4) {
        return false;
    }

    let mut bucket: Option<LinePlotDecimationBucket> = None;
    let mut emitted = 0usize;
    for (index, point) in visible.iter().copied().enumerate() {
        let Some(bucket_index) = line_plot_decimation_bucket_index(point, bounds, bucket_count)
        else {
            emitted += flush_line_plot_decimation_bucket(
                bucket.take(),
                points,
                series_instances,
                run_offset,
                run_len,
                plot,
                clip,
                bounds,
                line_width,
                color,
                aa_width,
                style_code,
                last_mapped,
                path_distance,
            );
            push_line_plot_renderer_point(
                points,
                series_instances,
                point,
                run_offset,
                run_len,
                plot,
                clip,
                bounds,
                line_width,
                color,
                aa_width,
                style_code,
                last_mapped,
                path_distance,
            );
            continue;
        };

        match bucket.as_mut() {
            Some(current) if current.bucket == bucket_index => current.push(index, point),
            Some(_) => {
                emitted += flush_line_plot_decimation_bucket(
                    bucket.take(),
                    points,
                    series_instances,
                    run_offset,
                    run_len,
                    plot,
                    clip,
                    bounds,
                    line_width,
                    color,
                    aa_width,
                    style_code,
                    last_mapped,
                    path_distance,
                );
                bucket = Some(LinePlotDecimationBucket::new(bucket_index, index, point));
            }
            None => {
                bucket = Some(LinePlotDecimationBucket::new(bucket_index, index, point));
            }
        }
    }
    emitted += flush_line_plot_decimation_bucket(
        bucket,
        points,
        series_instances,
        run_offset,
        run_len,
        plot,
        clip,
        bounds,
        line_width,
        color,
        aa_width,
        style_code,
        last_mapped,
        path_distance,
    );
    emitted < visible.len()
}

fn push_strided_line_plot_renderer_series(
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    visible: &[[f32; 2]],
    plot: [f32; 4],
    clip: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    aa_width: f32,
    max_segments_per_series: usize,
    style_code: f32,
    run_offset: &mut usize,
    run_len: &mut usize,
    last_mapped: &mut Option<[f32; 2]>,
    path_distance: &mut f32,
) -> bool {
    let target_points = line_plot_fast_decimation_target_points(plot, max_segments_per_series);
    if visible.len() <= target_points {
        return false;
    }
    let target_segments = target_points.saturating_sub(1).max(1);
    let source_segments = visible.len().saturating_sub(1).max(1);
    let stride = ((source_segments + target_segments - 1) / target_segments).max(1);
    if stride <= 1 {
        return false;
    }

    let mut last_index = 0usize;
    for idx in (0..visible.len()).step_by(stride) {
        push_line_plot_renderer_point(
            points,
            series_instances,
            visible[idx],
            run_offset,
            run_len,
            plot,
            clip,
            bounds,
            line_width,
            color,
            aa_width,
            style_code,
            last_mapped,
            path_distance,
        );
        last_index = idx;
    }
    if last_index != visible.len() - 1 {
        push_line_plot_renderer_point(
            points,
            series_instances,
            visible[visible.len() - 1],
            run_offset,
            run_len,
            plot,
            clip,
            bounds,
            line_width,
            color,
            aa_width,
            style_code,
            last_mapped,
            path_distance,
        );
    }
    true
}

fn line_plot_fast_decimation_target_points(
    plot: [f32; 4],
    max_segments_per_series: usize,
) -> usize {
    let pixel_columns = plot[2].ceil().max(0.0) as usize;
    if pixel_columns == 0 {
        return 0;
    }
    let pixel_target = pixel_columns.saturating_mul(2).saturating_add(2);
    pixel_target.clamp(2, max_segments_per_series.saturating_add(1).max(2))
}

fn line_plot_decimation_bucket_count(plot: [f32; 4], max_segments_per_series: usize) -> usize {
    let pixel_columns = plot[2].ceil().max(0.0) as usize;
    if pixel_columns == 0 {
        return 0;
    }
    let max_buckets = max_segments_per_series.saturating_sub(1).max(1) / 4;
    pixel_columns.min(max_buckets.max(1))
}

fn line_plot_decimation_bucket_index(
    point: [f32; 2],
    bounds: LinePlotBounds,
    bucket_count: usize,
) -> Option<usize> {
    if bucket_count == 0 || !point[0].is_finite() || !point[1].is_finite() {
        return None;
    }
    let x_range = (bounds.x_max - bounds.x_min).max(f32::EPSILON);
    let tx = ((point[0] - bounds.x_min) / x_range).clamp(0.0, 1.0);
    Some(((tx * bucket_count as f32).floor() as usize).min(bucket_count - 1))
}

#[derive(Clone, Copy, Debug)]
struct LinePlotDecimationBucket {
    bucket: usize,
    first: (usize, [f32; 2]),
    min_y: (usize, [f32; 2]),
    max_y: (usize, [f32; 2]),
    last: (usize, [f32; 2]),
}

impl LinePlotDecimationBucket {
    fn new(bucket: usize, index: usize, point: [f32; 2]) -> Self {
        Self {
            bucket,
            first: (index, point),
            min_y: (index, point),
            max_y: (index, point),
            last: (index, point),
        }
    }

    fn push(&mut self, index: usize, point: [f32; 2]) {
        if point[1] < self.min_y.1[1] {
            self.min_y = (index, point);
        }
        if point[1] > self.max_y.1[1] {
            self.max_y = (index, point);
        }
        self.last = (index, point);
    }

    fn ordered_points(self) -> [Option<[f32; 2]>; 4] {
        let mut candidates = [self.first, self.min_y, self.max_y, self.last];
        candidates.sort_by_key(|(index, _)| *index);
        let mut out = [None; 4];
        let mut out_len = 0usize;
        let mut previous_index = None;
        for (index, point) in candidates {
            if previous_index == Some(index) {
                continue;
            }
            out[out_len] = Some(point);
            out_len += 1;
            previous_index = Some(index);
        }
        out
    }
}

fn flush_line_plot_decimation_bucket(
    bucket: Option<LinePlotDecimationBucket>,
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    run_offset: &mut usize,
    run_len: &mut usize,
    plot: [f32; 4],
    clip: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    aa_width: f32,
    style_code: f32,
    last_mapped: &mut Option<[f32; 2]>,
    path_distance: &mut f32,
) -> usize {
    let Some(bucket) = bucket else {
        return 0;
    };
    let mut emitted = 0usize;
    for point in bucket.ordered_points().into_iter().flatten() {
        push_line_plot_renderer_point(
            points,
            series_instances,
            point,
            run_offset,
            run_len,
            plot,
            clip,
            bounds,
            line_width,
            color,
            aa_width,
            style_code,
            last_mapped,
            path_distance,
        );
        emitted += 1;
    }
    emitted
}

fn push_line_plot_renderer_point(
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    point: [f32; 2],
    run_offset: &mut usize,
    run_len: &mut usize,
    plot: [f32; 4],
    clip: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    aa_width: f32,
    style_code: f32,
    last_mapped: &mut Option<[f32; 2]>,
    path_distance: &mut f32,
) {
    if style_code < 0.5 {
        if !point[0].is_finite() || !point[1].is_finite() {
            flush_line_plot_renderer_run(
                points,
                series_instances,
                run_offset,
                run_len,
                plot,
                clip,
                bounds,
                line_width,
                color,
                aa_width,
                style_code,
            );
            *last_mapped = None;
            *path_distance = 0.0;
            return;
        }
        if *run_len == 0 {
            *run_offset = points.len();
        }
        points.push(LinePlotPointGpu {
            data: [point[0], point[1], 0.0, 0.0],
        });
        *run_len += 1;
        return;
    }

    let mapped = map_line_plot_point(point, plot, bounds);
    let Some(mapped) = mapped else {
        flush_line_plot_renderer_run(
            points,
            series_instances,
            run_offset,
            run_len,
            plot,
            clip,
            bounds,
            line_width,
            color,
            aa_width,
            style_code,
        );
        *last_mapped = None;
        *path_distance = 0.0;
        return;
    };
    if *run_len == 0 {
        *run_offset = points.len();
        *path_distance = 0.0;
    } else if let Some(previous) = *last_mapped {
        let dx = mapped[0] - previous[0];
        let dy = mapped[1] - previous[1];
        *path_distance += (dx * dx + dy * dy).sqrt();
    }
    points.push(LinePlotPointGpu {
        data: [point[0], point[1], *path_distance, 0.0],
    });
    *run_len += 1;
    *last_mapped = Some(mapped);
}

fn flush_line_plot_renderer_run(
    points: &mut Vec<LinePlotPointGpu>,
    series_instances: &mut Vec<LinePlotSeriesInstance>,
    run_offset: &mut usize,
    run_len: &mut usize,
    plot: [f32; 4],
    clip: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    aa_width: f32,
    style_code: f32,
) {
    if *run_len >= 2 {
        series_instances.push(LinePlotSeriesInstance {
            plot,
            clip,
            bounds: [bounds.x_min, bounds.x_max, bounds.y_min, bounds.y_max],
            color,
            params: [line_width, *run_offset as f32, *run_len as f32, aa_width],
            style: [style_code, 0.0, 0.0, 0.0],
        });
    } else {
        points.truncate(*run_offset);
    }
    *run_offset = points.len();
    *run_len = 0;
}

fn emit_line_plot(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    styled_bg: Option<[f32; 4]>,
    styled_border: Option<[f32; 4]>,
    radii: [f32; 4],
    border_w: f32,
) {
    let [_, _, w, h] = rect;
    emit_bordered_rect_radii(
        out,
        rect,
        styled_border.unwrap_or(theme.border),
        styled_bg.unwrap_or(theme.surface),
        radii,
        border_w,
    );
    if w <= 2.0 || h <= 2.0 {
        return;
    }

    let plot = line_plot_plot_rect(node, sf, rect);
    let plot_fill = mix(styled_bg.unwrap_or(theme.surface), theme.background, 0.18);
    out.push(inst_radii(plot, plot_fill, [2.0 * sf; 4]));

    let bounds = match line_plot_resolved_bounds(node) {
        Some(bounds) => bounds,
        None => {
            emit_line_plot_grid(
                out,
                plot,
                theme,
                sf,
                node.props.line_plot_show_grid,
                node.props.line_plot_show_axes,
                node.props.line_plot_show_ticks,
                None,
                &[],
                &[],
            );
            emit_line_plot_toolbar(out, node, theme, sf, rect);
            return;
        }
    };

    let tick_count = node.props.line_plot_tick_count.clamp(2, 9);
    let x_ticks = line_plot_ticks(bounds.x_min, bounds.x_max, tick_count);
    let y_ticks = line_plot_ticks(bounds.y_min, bounds.y_max, tick_count);
    emit_line_plot_grid(
        out,
        plot,
        theme,
        sf,
        node.props.line_plot_show_grid,
        node.props.line_plot_show_axes,
        node.props.line_plot_show_ticks,
        Some(bounds),
        &x_ticks,
        &y_ticks,
    );

    let line_width = (node.props.line_plot_line_width.max(0.5) * sf)
        .max(1.0)
        .min(plot[3].max(1.0) * 0.10);
    let use_line_plot_renderer = line_plot_gpu_renderer_enabled();
    for (series_index, series) in node.props.line_plot_series.iter().enumerate() {
        let series_points = series.logical_points();
        if series_points.len() < 2 {
            continue;
        }
        if use_line_plot_renderer && line_plot_renderer_style_code(&series.line_style).is_some() {
            continue;
        }
        let color = series
            .color
            .as_ref()
            .map(|color| color.resolve(theme))
            .unwrap_or(LINE_PLOT_PALETTE[series_index % LINE_PLOT_PALETTE.len()]);
        emit_line_plot_series(
            out,
            series_points,
            plot,
            bounds,
            line_width,
            color,
            &series.line_style,
        );
    }
    emit_line_plot_legend(out, node, theme, sf, plot);
    emit_line_plot_hover(out, node, theme, sf, plot);
    emit_line_plot_selection_rect(out, node, theme, sf, plot);
    emit_line_plot_toolbar(out, node, theme, sf, rect);
}

pub(crate) fn histogram_plot_rect(node: &WidgetNode, sf: f32, rect: [f32; 4]) -> [f32; 4] {
    let base_pad = 10.0 * sf;
    let show_ticks = node.props.histogram.show_ticks && rect[2] >= 220.0 && rect[3] >= 150.0;
    let show_axis_labels = node.props.histogram.show_axes && rect[2] >= 260.0 && rect[3] >= 205.0;
    let show_toolbar = histogram_toolbar_enabled(node, rect);
    let left = if node.props.histogram.show_axes || show_ticks {
        if show_axis_labels {
            48.0 * sf
        } else {
            34.0 * sf
        }
    } else {
        base_pad
    };
    let bottom = if node.props.histogram.show_axes || show_ticks {
        if show_axis_labels {
            42.0 * sf
        } else {
            28.0 * sf
        }
    } else {
        base_pad
    };
    let x = rect[0] + left;
    let top = if show_toolbar { 44.0 * sf } else { base_pad };
    let y = rect[1] + top;
    let w = (rect[2] - left - base_pad).max(1.0);
    let h = (rect[3] - top - bottom).max(1.0);
    [x, y, w, h]
}

fn histogram_toolbar_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.histogram.show_toolbar && rect[2] >= 190.0 && rect[3] >= 150.0
}

fn histogram_ticks_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.histogram.show_ticks && rect[2] >= 220.0 && rect[3] >= 150.0
}

fn histogram_axis_labels_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.histogram.show_axes && rect[2] >= 260.0 && rect[3] >= 205.0
}

pub(crate) fn histogram_resolved_bounds(node: &WidgetNode) -> Option<LinePlotBounds> {
    if !node.props.histogram.auto_fit {
        if let (Some(x_min), Some(x_max), Some(y_min), Some(y_max)) = (
            node.props.histogram.x_min,
            node.props.histogram.x_max,
            node.props.histogram.y_min,
            node.props.histogram.y_max,
        ) {
            if x_min.is_finite()
                && x_max.is_finite()
                && y_min.is_finite()
                && y_max.is_finite()
                && x_max > x_min
                && y_max > y_min
            {
                return Some(LinePlotBounds {
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                });
            }
        }
    }
    histogram_data_bounds(node)
}

fn histogram_data_bounds(node: &WidgetNode) -> Option<LinePlotBounds> {
    let histogram = &node.props.histogram;
    if histogram.edges.len() != histogram.counts.len().saturating_add(1) {
        return None;
    }
    let x_min = *histogram.edges.first()?;
    let x_max = *histogram.edges.last()?;
    if !x_min.is_finite() || !x_max.is_finite() || x_max <= x_min {
        return None;
    }
    let mut y_max = histogram
        .counts
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .fold(0.0_f32, f32::max);
    if y_max <= 0.0 {
        y_max = 1.0;
    }
    y_max *= 1.08;
    Some(LinePlotBounds {
        x_min,
        x_max,
        y_min: 0.0,
        y_max,
    })
}

fn pie_chart_toolbar_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.pie_chart.show_toolbar && rect[2] >= 260.0 && rect[3] >= 170.0
}

fn pie_chart_chart_rect(node: &WidgetNode, sf: f32, rect: [f32; 4]) -> [f32; 4] {
    let pad = 14.0 * sf;
    let title_h = if node.props.pie_chart.title.is_some() {
        24.0 * sf
    } else {
        0.0
    };
    let toolbar_h = if pie_chart_toolbar_enabled(node, rect) {
        34.0 * sf
    } else {
        0.0
    };
    let legend = node.props.pie_chart.show_legend
        && node.props.pie_chart.legend_position != "none"
        && !node.props.pie_chart.slices.is_empty();
    let mut x = rect[0] + pad;
    let mut y = rect[1] + pad + title_h + toolbar_h;
    let mut w = (rect[2] - pad * 2.0).max(1.0);
    let mut h = (rect[3] - pad * 2.0 - title_h - toolbar_h).max(1.0);
    if legend {
        match node.props.pie_chart.legend_position.as_str() {
            "right" if w > 320.0 * sf => w -= (150.0 * sf).min(w * 0.36),
            "left" if w > 320.0 * sf => {
                let legend_w = (150.0 * sf).min(w * 0.36);
                x += legend_w;
                w -= legend_w;
            }
            "bottom" if h > 260.0 * sf => h -= (70.0 * sf).min(h * 0.28),
            "top" if h > 260.0 * sf => {
                let legend_h = (70.0 * sf).min(h * 0.28);
                y += legend_h;
                h -= legend_h;
            }
            _ => {}
        }
    }
    [x, y, w, h]
}

fn pie_chart_legend_rect(node: &WidgetNode, sf: f32, rect: [f32; 4], chart: [f32; 4]) -> [f32; 4] {
    let pad = 14.0 * sf;
    match node.props.pie_chart.legend_position.as_str() {
        "left" => [
            rect[0] + pad,
            chart[1],
            (chart[0] - rect[0] - pad * 1.5).max(1.0),
            chart[3],
        ],
        "bottom" => [
            chart[0],
            chart[1] + chart[3] + 8.0 * sf,
            chart[2],
            (rect[1] + rect[3] - chart[1] - chart[3] - pad * 1.5).max(1.0),
        ],
        "top" => [
            chart[0],
            rect[1] + pad,
            chart[2],
            (chart[1] - rect[1] - pad * 1.5).max(1.0),
        ],
        _ => [
            chart[0] + chart[2] + 10.0 * sf,
            chart[1],
            (rect[0] + rect[2] - chart[0] - chart[2] - pad * 1.5).max(1.0),
            chart[3],
        ],
    }
}

fn pie_slice_angles(start_cursor: f32, sweep: f32, clockwise: bool) -> (f32, f32, f32, f32) {
    if clockwise {
        let start = start_cursor;
        let end = start_cursor + sweep;
        (start, end, start + sweep * 0.5, end)
    } else {
        let start = start_cursor - sweep;
        let end = start_cursor;
        (start, end, start_cursor - sweep * 0.5, start)
    }
}

struct PieChartLabelLayout {
    text: String,
    screen_x: f32,
    screen_y: f32,
    rect: [f32; 4],
}

fn pie_chart_slice_label_text(
    slice: &crate::document::PieChartSliceProp,
    percent: f32,
    value_mode: &str,
) -> String {
    let value_text = match value_mode {
        "value" => format!("{:.0}", slice.value),
        "both" => format!("{:.0} ({:.0}%)", slice.value, percent * 100.0),
        "none" => slice.label.clone(),
        _ => format!("{:.0}%", percent * 100.0),
    };
    if value_mode == "none" {
        slice.label.clone()
    } else {
        format!("{} {}", slice.label, value_text)
    }
}

fn pie_chart_slice_label_layouts(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    plot: [f32; 4],
) -> Vec<PieChartLabelLayout> {
    let mut labels = Vec::new();
    let label_mode = node.props.pie_chart.label_mode.as_str();
    if !node.props.pie_chart.show_labels || matches!(label_mode, "none" | "legend") {
        return labels;
    }

    let total = node.props.pie_chart.total.max(f32::EPSILON);
    let size = plot[2].min(plot[3]).max(1.0);
    let mut cursor = node.props.pie_chart.start_angle.to_radians();
    for slice in &node.props.pie_chart.slices {
        let percent = slice.value / total;
        let sweep = percent * std::f32::consts::TAU;
        let (_, _, mid, next_cursor) =
            pie_slice_angles(cursor, sweep, node.props.pie_chart.clockwise);
        if percent < 0.075 && label_mode == "auto" {
            cursor = next_cursor;
            continue;
        }

        let outer = size * 0.5;
        let inner = if node.props.pie_chart.donut {
            outer * node.props.pie_chart.inner_radius
        } else {
            0.0
        };
        let r = if label_mode == "outside" {
            inner + (outer - inner) * 0.88
        } else {
            inner + (outer - inner) * 0.58
        };
        let text = pie_chart_slice_label_text(slice, percent, &node.props.pie_chart.value_mode);
        let screen_x = plot[0] + plot[2] * 0.5 + mid.cos() * r;
        let screen_y = plot[1] + plot[3] * 0.5 + mid.sin() * r;
        let font_size_lp = pie_chart_label_font_size(node).unwrap_or(10.0).max(8.0);
        let font_size = font_size_lp * sf;
        let text_style = TextStyle {
            font_size: Some(font_size_lp),
            ..node.style.text.clone()
        };
        let text_w = measure_text_for_layout(&text, &text_style, theme).width * sf + 14.0 * sf;
        let label_w = text_w.min(plot[2] * 0.62).max(42.0 * sf);
        let label_h = font_size * 1.55;
        let label_left = (screen_x - label_w * 0.5)
            .max(plot[0])
            .min((plot[0] + plot[2] - label_w).max(plot[0]));
        let label_top = (screen_y - label_h * 0.5)
            .max(plot[1])
            .min((plot[1] + plot[3] - label_h).max(plot[1]));
        labels.push(PieChartLabelLayout {
            text,
            screen_x,
            screen_y,
            rect: [label_left, label_top, label_w, label_h],
        });
        cursor = next_cursor;
    }
    labels
}

fn pie_chart_plot_rect(chart_area: [f32; 4], sf: f32) -> [f32; 4] {
    let pie_pad = (8.0 * sf).min(chart_area[2].min(chart_area[3]) * 0.12);
    let size = (chart_area[2] - pie_pad * 2.0)
        .min(chart_area[3] - pie_pad * 2.0)
        .max(1.0);
    [
        chart_area[0] + (chart_area[2] - size) * 0.5,
        chart_area[1] + (chart_area[3] - size) * 0.5,
        size,
        size,
    ]
}

fn emit_pie_chart(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    styled_bg: Option<[f32; 4]>,
    styled_border: Option<[f32; 4]>,
    radii: [f32; 4],
    border_w: f32,
) {
    emit_bordered_rect_radii(
        out,
        rect,
        styled_border.unwrap_or(theme.border),
        styled_bg.unwrap_or(theme.surface),
        radii,
        border_w,
    );
    let chart_area = pie_chart_chart_rect(node, sf, rect);
    let plot_fill = mix(styled_bg.unwrap_or(theme.surface), theme.background, 0.12);

    let plot = pie_chart_plot_rect(chart_area, sf);
    let size = plot[2].min(plot[3]).max(1.0);
    let total = node.props.pie_chart.total;
    if total <= 0.0 || node.props.pie_chart.slices.is_empty() {
        emit_pie_chart_toolbar(out, node, theme, sf, rect);
        return;
    }
    let tau = std::f32::consts::TAU;
    let mut cursor = node.props.pie_chart.start_angle.to_radians();
    let selected = node.props.pie_chart.selected.as_deref();
    for (index, slice) in node.props.pie_chart.slices.iter().enumerate() {
        let sweep = (slice.value / total).clamp(0.0, 1.0) * tau;
        if sweep <= 0.0001 {
            continue;
        }
        let (start, end, _, next_cursor) =
            pie_slice_angles(cursor, sweep, node.props.pie_chart.clockwise);
        let is_selected = selected.is_some_and(|value| {
            value == slice.label || value.parse::<usize>().ok() == Some(index)
        });
        let mut color = slice
            .color
            .as_ref()
            .map(|color| color.resolve(theme))
            .unwrap_or_else(|| palette_color(index, theme));
        if is_selected {
            color = mix(color, theme.text, 0.16);
        }
        out.push(inst_pie_slice(
            plot,
            color,
            start,
            end,
            if node.props.pie_chart.donut {
                node.props.pie_chart.inner_radius
            } else {
                0.0
            },
        ));
        cursor = next_cursor;
    }
    let border = styled_border.unwrap_or(theme.border);
    out.push(inst_outline_ring_clipped(
        plot,
        [border[0], border[1], border[2], 0.55],
        [size * 0.5; 4],
        1.0 * sf,
        [-1.0, -1.0, plot[2] + 1.0, plot[3] + 1.0],
    ));
    if node.props.pie_chart.donut {
        let inner = size * node.props.pie_chart.inner_radius;
        let hole = [
            plot[0] + (size - inner) * 0.5,
            plot[1] + (size - inner) * 0.5,
            inner,
            inner,
        ];
        out.push(inst_radii(hole, plot_fill, [inner * 0.5; 4]));
    }
    for label in pie_chart_slice_label_layouts(node, theme, sf, plot) {
        let chip = mix(theme.background, theme.surface, 0.18);
        out.push(inst_radii(
            label.rect,
            [chip[0], chip[1], chip[2], 0.62],
            [label.rect[3] * 0.5; 4],
        ));
    }
    if node.props.pie_chart.show_legend && node.props.pie_chart.legend_position != "none" {
        let legend = pie_chart_legend_rect(node, sf, rect, chart_area);
        let legend_font_size = pie_chart_label_font_size(node).unwrap_or(10.0).max(10.0);
        let line_h = legend_font_size * 1.3 * sf;
        let row_h = (line_h + 6.0 * sf).max(20.0 * sf);
        let swatch = 10.0 * sf;
        for (index, slice) in node.props.pie_chart.slices.iter().enumerate() {
            let y = legend[1] + 8.0 * sf + index as f32 * row_h;
            if y + row_h > legend[1] + legend[3] {
                break;
            }
            let color = slice
                .color
                .as_ref()
                .map(|color| color.resolve(theme))
                .unwrap_or_else(|| palette_color(index, theme));
            out.push(inst_radii(
                [
                    legend[0] + 7.0 * sf,
                    y + (row_h - swatch).max(0.0) * 0.5,
                    swatch,
                    swatch,
                ],
                color,
                [3.0 * sf; 4],
            ));
        }
    }
    emit_pie_chart_toolbar(out, node, theme, sf, rect);
}

fn pie_chart_toolbar_buttons(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
) -> Vec<(&'static str, [f32; 4], bool)> {
    if !pie_chart_toolbar_enabled(node, rect) {
        return Vec::new();
    }
    let pad = 10.0 * sf;
    let button = 24.0 * sf;
    let gap = 5.0 * sf;
    let labels = ["Fit", "Pan", "Zoom", "Box", "Grid", "Axes"];
    let total = button * labels.len() as f32 + gap * (labels.len().saturating_sub(1)) as f32;
    let y = rect[1] + pad;
    let mut x = rect[0] + rect[2] - pad - total;
    let mut buttons = Vec::with_capacity(labels.len());
    for label in labels {
        buttons.push((label, [x, y, button, button], true));
        x += button + gap;
    }
    buttons
}

fn emit_pie_chart_toolbar(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) {
    for (label, button, active) in pie_chart_toolbar_buttons(node, sf, rect) {
        let mut fill = if active {
            mix(theme.surface_alt, theme.accent, 0.18)
        } else {
            mix(theme.surface_alt, theme.surface, 0.45)
        };
        fill[3] = fill[3].min(0.88);
        let mut border = if active {
            mix(theme.border, theme.accent, 0.50)
        } else {
            mix(theme.border, theme.muted_text, 0.20)
        };
        border[3] = border[3].min(0.68);
        emit_bordered_rect_radii(out, button, border, fill, [4.0 * sf; 4], 1.0 * sf);
        let mut icon = if active {
            mix(theme.text, theme.accent, 0.24)
        } else {
            mix(theme.muted_text, theme.text, 0.20)
        };
        icon[3] = icon[3].min(0.92);
        emit_line_plot_toolbar_icon(out, label, button, icon, sf);
    }
}

fn palette_color(index: usize, theme: &Theme) -> [f32; 4] {
    const COLORS: [[f32; 4]; 8] = [
        [0.35, 0.66, 1.0, 1.0],
        [0.45, 0.86, 0.69, 1.0],
        [1.0, 0.82, 0.42, 1.0],
        [0.95, 0.42, 0.50, 1.0],
        [0.70, 0.53, 1.0, 1.0],
        [1.0, 0.62, 0.26, 1.0],
        [0.30, 0.82, 0.88, 1.0],
        [0.64, 0.90, 0.20, 1.0],
    ];
    COLORS
        .get(index % COLORS.len())
        .copied()
        .unwrap_or(theme.accent)
}

fn emit_histogram(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    styled_bg: Option<[f32; 4]>,
    styled_border: Option<[f32; 4]>,
    styled_accent: Option<[f32; 4]>,
    radii: [f32; 4],
    border_w: f32,
) {
    emit_bordered_rect_radii(
        out,
        rect,
        styled_border.unwrap_or(theme.border),
        styled_bg.unwrap_or(theme.surface),
        radii,
        border_w,
    );
    if rect[2] <= 2.0 || rect[3] <= 2.0 {
        return;
    }
    let plot = histogram_plot_rect(node, sf, rect);
    let plot_fill = mix(styled_bg.unwrap_or(theme.surface), theme.background, 0.18);
    out.push(inst_radii(plot, plot_fill, [2.0 * sf; 4]));

    let Some(bounds) = histogram_resolved_bounds(node) else {
        emit_line_plot_grid(
            out,
            plot,
            theme,
            sf,
            node.props.histogram.show_grid,
            node.props.histogram.show_axes,
            node.props.histogram.show_ticks,
            None,
            &[],
            &[],
        );
        emit_histogram_toolbar(out, node, theme, sf, rect);
        return;
    };
    let tick_count = node.props.histogram.tick_count.clamp(2, 9);
    let x_ticks = line_plot_ticks(bounds.x_min, bounds.x_max, tick_count);
    let y_ticks = line_plot_ticks(bounds.y_min, bounds.y_max, tick_count);
    emit_line_plot_grid(
        out,
        plot,
        theme,
        sf,
        node.props.histogram.show_grid,
        node.props.histogram.show_axes,
        node.props.histogram.show_ticks,
        Some(bounds),
        &x_ticks,
        &y_ticks,
    );

    let color = node
        .props
        .histogram
        .color
        .as_ref()
        .map(|color| color.resolve(theme))
        .unwrap_or(styled_accent.unwrap_or(theme.accent));
    let gap = node.props.histogram.bar_gap.max(0.0) * sf;
    let span = (bounds.x_max - bounds.x_min).max(f32::EPSILON);
    for (index, count) in node.props.histogram.counts.iter().copied().enumerate() {
        if !count.is_finite() || count <= 0.0 {
            continue;
        }
        let Some(left) = node.props.histogram.edges.get(index).copied() else {
            continue;
        };
        let Some(right) = node.props.histogram.edges.get(index + 1).copied() else {
            continue;
        };
        if right <= left || right < bounds.x_min || left > bounds.x_max {
            continue;
        }
        let clipped_left = left.max(bounds.x_min);
        let clipped_right = right.min(bounds.x_max);
        let x0 = plot[0] + ((clipped_left - bounds.x_min) / span).clamp(0.0, 1.0) * plot[2];
        let x1 = plot[0] + ((clipped_right - bounds.x_min) / span).clamp(0.0, 1.0) * plot[2];
        let width = (x1 - x0).max(0.0);
        if width <= 0.5 {
            continue;
        }
        let inset = gap.min(width * 0.42);
        let bar_w = (width - inset * 2.0).max(0.75);
        let visible_bottom = bounds.y_min.max(0.0);
        let visible_top = count.min(bounds.y_max);
        if visible_top <= visible_bottom {
            continue;
        }
        let y_span = (bounds.y_max - bounds.y_min).max(f32::EPSILON);
        let t0 = ((visible_bottom - bounds.y_min) / y_span).clamp(0.0, 1.0);
        let t1 = ((visible_top - bounds.y_min) / y_span).clamp(0.0, 1.0);
        let bar_h = (plot[3] * (t1 - t0)).max(0.75);
        let bar_x = x0 + inset;
        let bar_y = plot[1] + plot[3] * (1.0 - t1);
        out.push(inst_radii(
            [bar_x, bar_y, bar_w, bar_h],
            color,
            [2.0 * sf, 2.0 * sf, 0.0, 0.0],
        ));
    }
    emit_histogram_selection_rect(out, node, theme, sf, plot);
    emit_histogram_toolbar(out, node, theme, sf, rect);
}

fn histogram_toolbar_buttons(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
) -> Vec<(&'static str, [f32; 4], bool)> {
    if !histogram_toolbar_enabled(node, rect) {
        return Vec::new();
    }
    let pad = 10.0 * sf;
    let button = 24.0 * sf;
    let gap = 5.0 * sf;
    let labels = ["Fit", "Pan", "Zoom", "Box", "Grid", "Axes"];
    let total = button * labels.len() as f32 + gap * (labels.len().saturating_sub(1)) as f32;
    let y = rect[1] + pad;
    let mut x = rect[0] + rect[2] - pad - total;
    let mut buttons = Vec::with_capacity(labels.len());
    for label in labels {
        let active = match label {
            "Pan" => node.props.histogram.interaction == "pan",
            "Zoom" => node.props.histogram.interaction == "zoom",
            "Box" => node.props.histogram.interaction == "box_zoom",
            "Grid" => node.props.histogram.show_grid,
            "Axes" => node.props.histogram.show_axes || node.props.histogram.show_ticks,
            _ => true,
        };
        buttons.push((label, [x, y, button, button], active));
        x += button + gap;
    }
    buttons
}

fn emit_histogram_selection_rect(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    plot: [f32; 4],
) {
    let Some(raw) = node.props.histogram.selection_rect else {
        return;
    };
    let x0 = raw[0].min(raw[2]).clamp(plot[0], plot[0] + plot[2]);
    let x1 = raw[0].max(raw[2]).clamp(plot[0], plot[0] + plot[2]);
    let y0 = raw[1].min(raw[3]).clamp(plot[1], plot[1] + plot[3]);
    let y1 = raw[1].max(raw[3]).clamp(plot[1], plot[1] + plot[3]);
    let rect = [x0, y0, x1 - x0, y1 - y0];
    if rect[2] < 2.0 * sf || rect[3] < 2.0 * sf {
        return;
    }
    let mut fill = mix(theme.accent, theme.surface, 0.24);
    fill[3] = 0.18;
    let mut border = mix(theme.accent, theme.text, 0.20);
    border[3] = 0.82;
    emit_bordered_rect_radii(out, rect, border, fill, [2.0 * sf; 4], 1.0 * sf);
}

pub(crate) fn histogram_toolbar_hit(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
    pos: [f32; 2],
) -> Option<&'static str> {
    for (label, button, _) in histogram_toolbar_buttons(node, sf, rect) {
        if pos[0] >= button[0]
            && pos[0] < button[0] + button[2]
            && pos[1] >= button[1]
            && pos[1] < button[1] + button[3]
        {
            return Some(label);
        }
    }
    None
}

fn emit_histogram_toolbar(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) {
    for (label, button, active) in histogram_toolbar_buttons(node, sf, rect) {
        let mut fill = if active {
            mix(theme.surface_alt, theme.accent, 0.18)
        } else {
            mix(theme.surface_alt, theme.surface, 0.45)
        };
        fill[3] = fill[3].min(0.88);
        let mut border = if active {
            mix(theme.border, theme.accent, 0.50)
        } else {
            mix(theme.border, theme.muted_text, 0.20)
        };
        border[3] = border[3].min(0.68);
        emit_bordered_rect_radii(out, button, border, fill, [4.0 * sf; 4], 1.0 * sf);
        let mut icon = if active {
            mix(theme.text, theme.accent, 0.24)
        } else {
            mix(theme.muted_text, theme.text, 0.20)
        };
        icon[3] = icon[3].min(0.92);
        emit_line_plot_toolbar_icon(out, label, button, icon, sf);
    }
}

pub(crate) fn histogram_text_labels(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) -> Vec<LinePlotTextLabel> {
    let mut labels = Vec::new();
    let plot = histogram_plot_rect(node, sf, rect);
    let tick_color = mix(theme.muted_text, theme.text, 0.18);
    let tick_color = Some([tick_color[0], tick_color[1], tick_color[2]]);

    if histogram_axis_labels_enabled(node, rect) {
        let axis_color = mix(theme.muted_text, theme.text, 0.72);
        let axis_color = Some([axis_color[0], axis_color[1], axis_color[2]]);
        if let Some(label) = node.props.histogram.x_label.as_deref() {
            labels.push(LinePlotTextLabel {
                text: label.to_string(),
                screen_x: plot[0] + plot[2] * 0.5,
                screen_y: rect[1] + rect[3] - 11.0 * sf,
                is_title: true,
                anchor: "plot-x-label",
                color: axis_color,
                font_size: Some(LINE_PLOT_AXIS_LABEL_FONT_SIZE_LP),
                clip_rect: None,
            });
        }
        if let Some(label) = node.props.histogram.y_label.as_deref() {
            labels.push(LinePlotTextLabel {
                text: label.to_string(),
                screen_x: rect[0] + 18.0 * sf,
                screen_y: plot[1] + plot[3] * 0.5,
                is_title: true,
                anchor: "plot-y-label",
                color: axis_color,
                font_size: Some(LINE_PLOT_AXIS_LABEL_FONT_SIZE_LP),
                clip_rect: None,
            });
        }
    }

    let Some(bounds) = histogram_resolved_bounds(node) else {
        return labels;
    };
    if histogram_ticks_enabled(node, rect) {
        let tick_count = node.props.histogram.tick_count.clamp(2, 9);
        let x_ticks = line_plot_ticks(bounds.x_min, bounds.x_max, tick_count);
        let y_ticks = line_plot_ticks(bounds.y_min, bounds.y_max, tick_count);
        let x_step = x_ticks
            .windows(2)
            .next()
            .map(|pair| (pair[1] - pair[0]).abs())
            .unwrap_or_else(|| (bounds.x_max - bounds.x_min).abs());
        let y_step = y_ticks
            .windows(2)
            .next()
            .map(|pair| (pair[1] - pair[0]).abs())
            .unwrap_or_else(|| (bounds.y_max - bounds.y_min).abs());
        for tick in x_ticks {
            let t = ((tick - bounds.x_min) / (bounds.x_max - bounds.x_min).max(f32::EPSILON))
                .clamp(0.0, 1.0);
            labels.push(LinePlotTextLabel {
                text: format_line_plot_tick(tick, x_step),
                screen_x: plot[0] + plot[2] * t,
                screen_y: plot[1] + plot[3] + 7.0 * sf,
                is_title: false,
                anchor: "plot-x-tick",
                color: tick_color,
                font_size: Some(10.0),
                clip_rect: None,
            });
        }
        for tick in y_ticks {
            let t = ((tick - bounds.y_min) / (bounds.y_max - bounds.y_min).max(f32::EPSILON))
                .clamp(0.0, 1.0);
            labels.push(LinePlotTextLabel {
                text: format_line_plot_tick(tick, y_step),
                screen_x: plot[0] - 2.0 * sf,
                screen_y: plot[1] + plot[3] * (1.0 - t),
                is_title: false,
                anchor: "plot-y-tick",
                color: tick_color,
                font_size: Some(10.0),
                clip_rect: None,
            });
        }
    }

    labels
}

pub(crate) fn bar_chart_plot_rect(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) -> [f32; 4] {
    let base_pad = 10.0 * sf;
    let show_ticks = bar_chart_ticks_enabled(node, rect);
    let show_axis_labels = bar_chart_axis_labels_enabled(node, rect);
    let show_toolbar = bar_chart_toolbar_enabled(node, rect);
    let left = if node.props.bar_chart.show_axes || show_ticks {
        if bar_chart_is_horizontal(node) {
            let label_lane = if show_ticks {
                let text_style = TextStyle {
                    font_size: Some(10.0),
                    ..node.style.text.clone()
                };
                let max_label_width = node
                    .props
                    .bar_chart
                    .labels
                    .iter()
                    .map(|label| measure_text_for_layout(label, &text_style, theme).width)
                    .fold(0.0, f32::max);
                (max_label_width * sf + 8.0 * sf).clamp(36.0 * sf, 68.0 * sf)
            } else {
                0.0
            };
            let title_lane = if show_axis_labels && node.props.bar_chart.y_label.is_some() {
                20.0 * sf
            } else {
                0.0
            };
            (base_pad + title_lane + label_lane + 5.0 * sf).max(44.0 * sf)
        } else if show_axis_labels {
            48.0 * sf
        } else {
            34.0 * sf
        }
    } else {
        base_pad
    };
    let bottom = if node.props.bar_chart.show_axes || show_ticks {
        if show_axis_labels {
            42.0 * sf
        } else {
            28.0 * sf
        }
    } else {
        base_pad
    };
    let top = if show_toolbar { 44.0 * sf } else { base_pad };
    [
        rect[0] + left,
        rect[1] + top,
        (rect[2] - left - base_pad).max(1.0),
        (rect[3] - top - bottom).max(1.0),
    ]
}

fn bar_chart_toolbar_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.bar_chart.show_toolbar && rect[2] >= 145.0 && rect[3] >= 150.0
}

fn bar_chart_ticks_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.bar_chart.show_ticks && rect[2] >= 220.0 && rect[3] >= 150.0
}

fn bar_chart_axis_labels_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.bar_chart.show_axes && rect[2] >= 260.0 && rect[3] >= 205.0
}

fn bar_chart_is_horizontal(node: &WidgetNode) -> bool {
    node.props.bar_chart.orientation == "horizontal"
}

pub(crate) fn bar_chart_resolved_bounds(node: &WidgetNode) -> Option<LinePlotBounds> {
    let categories = node.props.bar_chart.labels.len();
    if categories == 0 || node.props.bar_chart.series.is_empty() {
        return None;
    }
    let (mut value_min, mut value_max) = if !node.props.bar_chart.auto_fit {
        match (
            node.props.bar_chart.value_min,
            node.props.bar_chart.value_max,
        ) {
            (Some(min), Some(max)) if min.is_finite() && max.is_finite() && max > min => (min, max),
            _ => bar_chart_data_value_bounds(node)?,
        }
    } else {
        bar_chart_data_value_bounds(node)?
    };
    if value_min == value_max {
        value_min -= 0.5;
        value_max += 0.5;
    }
    if bar_chart_is_horizontal(node) {
        Some(LinePlotBounds {
            x_min: value_min,
            x_max: value_max,
            y_min: 0.0,
            y_max: categories as f32,
        })
    } else {
        Some(LinePlotBounds {
            x_min: 0.0,
            x_max: categories as f32,
            y_min: value_min,
            y_max: value_max,
        })
    }
}

fn bar_chart_data_value_bounds(node: &WidgetNode) -> Option<(f32, f32)> {
    let mut min_value = 0.0_f32;
    let mut max_value = 0.0_f32;
    let mut any = false;
    for series in &node.props.bar_chart.series {
        for value in &series.values {
            if !value.is_finite() {
                continue;
            }
            if !any {
                min_value = min_value.min(*value);
                max_value = max_value.max(*value);
                any = true;
            } else {
                min_value = min_value.min(*value);
                max_value = max_value.max(*value);
            }
        }
    }
    if !any {
        return Some((0.0, 1.0));
    }
    if min_value >= 0.0 {
        max_value = if max_value <= 0.0 {
            1.0
        } else {
            max_value * 1.08
        };
        min_value = 0.0;
    } else if max_value <= 0.0 {
        min_value *= 1.08;
        max_value = 0.0;
    } else {
        let pad = (max_value - min_value).abs() * 0.06;
        min_value -= pad;
        max_value += pad;
    }
    Some((min_value, max_value))
}

#[derive(Debug, Clone)]
struct BarChartBarLayout {
    index: usize,
    category: String,
    series_index: usize,
    series_label: Option<String>,
    value: f32,
    rect: [f32; 4],
    center: [f32; 2],
    color: [f32; 4],
}

fn bar_chart_bar_layouts(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    plot: [f32; 4],
    bounds: LinePlotBounds,
    styled_accent: Option<[f32; 4]>,
) -> Vec<BarChartBarLayout> {
    let categories = node.props.bar_chart.labels.len();
    let series_count = node.props.bar_chart.series.len();
    if categories == 0 || series_count == 0 {
        return Vec::new();
    }
    let gap = node.props.bar_chart.bar_gap.max(0.0) * sf;
    let mut bars = Vec::with_capacity(categories.saturating_mul(series_count));
    if bar_chart_is_horizontal(node) {
        let group_h = plot[3] / categories as f32;
        let group_pad = gap.min(group_h * 0.22);
        let inner_h = (group_h - group_pad * 2.0).max(1.0);
        let series_gap = gap.min(inner_h * 0.24);
        let bar_h = ((inner_h - series_gap * series_count.saturating_sub(1) as f32)
            / series_count as f32)
            .max(0.75);
        let span = (bounds.x_max - bounds.x_min).max(f32::EPSILON);
        let zero_x = plot[0] + ((0.0 - bounds.x_min) / span).clamp(0.0, 1.0) * plot[2];
        for (index, category) in node.props.bar_chart.labels.iter().enumerate() {
            let group_y = plot[1] + group_h * index as f32 + group_pad;
            for (series_index, series) in node.props.bar_chart.series.iter().enumerate() {
                let Some(value) = series.values.get(index).copied() else {
                    continue;
                };
                if !value.is_finite() {
                    continue;
                }
                let clamped = value.clamp(bounds.x_min, bounds.x_max);
                let value_x = plot[0] + ((clamped - bounds.x_min) / span).clamp(0.0, 1.0) * plot[2];
                let x = zero_x.min(value_x);
                let w = (zero_x - value_x).abs().max(0.75);
                let y = group_y + series_index as f32 * (bar_h + series_gap);
                let color = series
                    .color
                    .as_ref()
                    .map(|color| color.resolve(theme))
                    .unwrap_or_else(|| {
                        if series_count == 1 {
                            styled_accent.unwrap_or(theme.accent)
                        } else {
                            palette_color(series_index, theme)
                        }
                    });
                bars.push(BarChartBarLayout {
                    index,
                    category: category.clone(),
                    series_index,
                    series_label: series.label.clone(),
                    value,
                    rect: [x, y, w, bar_h],
                    center: [x + w * 0.5, y + bar_h * 0.5],
                    color,
                });
            }
        }
    } else {
        let group_w = plot[2] / categories as f32;
        let group_pad = gap.min(group_w * 0.22);
        let inner_w = (group_w - group_pad * 2.0).max(1.0);
        let series_gap = gap.min(inner_w * 0.24);
        let bar_w = ((inner_w - series_gap * series_count.saturating_sub(1) as f32)
            / series_count as f32)
            .max(0.75);
        let span = (bounds.y_max - bounds.y_min).max(f32::EPSILON);
        let zero_y = plot[1] + plot[3] * (1.0 - ((0.0 - bounds.y_min) / span).clamp(0.0, 1.0));
        for (index, category) in node.props.bar_chart.labels.iter().enumerate() {
            let group_x = plot[0] + group_w * index as f32 + group_pad;
            for (series_index, series) in node.props.bar_chart.series.iter().enumerate() {
                let Some(value) = series.values.get(index).copied() else {
                    continue;
                };
                if !value.is_finite() {
                    continue;
                }
                let clamped = value.clamp(bounds.y_min, bounds.y_max);
                let value_y =
                    plot[1] + plot[3] * (1.0 - ((clamped - bounds.y_min) / span).clamp(0.0, 1.0));
                let y = zero_y.min(value_y);
                let h = (zero_y - value_y).abs().max(0.75);
                let x = group_x + series_index as f32 * (bar_w + series_gap);
                let color = series
                    .color
                    .as_ref()
                    .map(|color| color.resolve(theme))
                    .unwrap_or_else(|| {
                        if series_count == 1 {
                            styled_accent.unwrap_or(theme.accent)
                        } else {
                            palette_color(series_index, theme)
                        }
                    });
                bars.push(BarChartBarLayout {
                    index,
                    category: category.clone(),
                    series_index,
                    series_label: series.label.clone(),
                    value,
                    rect: [x, y, bar_w, h],
                    center: [x + bar_w * 0.5, y + h * 0.5],
                    color,
                });
            }
        }
    }
    bars
}

pub(crate) fn bar_chart_bar_at(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    pos: [f32; 2],
) -> Option<BarChartHoverProp> {
    let plot = bar_chart_plot_rect(node, theme, sf, rect);
    if pos[0] < plot[0]
        || pos[0] >= plot[0] + plot[2]
        || pos[1] < plot[1]
        || pos[1] >= plot[1] + plot[3]
    {
        return None;
    }
    let bounds = bar_chart_resolved_bounds(node)?;
    for bar in bar_chart_bar_layouts(node, theme, sf, plot, bounds, None) {
        let [x, y, w, h] = bar.rect;
        if pos[0] >= x && pos[0] < x + w && pos[1] >= y && pos[1] < y + h {
            return Some(BarChartHoverProp {
                index: bar.index,
                category: bar.category,
                series_index: bar.series_index,
                series_label: bar.series_label,
                value: bar.value,
                screen: bar.center,
            });
        }
    }
    None
}

fn bar_chart_readout_rect(screen: [f32; 2], plot: [f32; 4], sf: f32) -> [f32; 4] {
    let box_w = 190.0 * sf;
    let box_h = 30.0 * sf;
    let mut left = screen[0] + 10.0 * sf;
    let mut top = screen[1] - box_h - 8.0 * sf;
    if left + box_w > plot[0] + plot[2] {
        left = screen[0] - box_w - 10.0 * sf;
    }
    if top < plot[1] {
        top = screen[1] + 10.0 * sf;
    }
    [left, top, box_w, box_h]
}

fn part_style_text_rgb(style: &PartStyle, theme: &Theme) -> Option<[f32; 3]> {
    let color = style
        .text
        .color
        .as_ref()
        .or(style.visual.foreground.as_ref())?;
    let resolved = color.resolve(theme);
    Some([resolved[0], resolved[1], resolved[2]])
}

fn bar_chart_value_label_text_color(node: &WidgetNode, theme: &Theme) -> Option<[f32; 3]> {
    ["value-label", "label"].into_iter().find_map(|part| {
        base_part_style(&node.style, part).and_then(|style| part_style_text_rgb(style, theme))
    })
}

fn bar_chart_value_label_font_size(node: &WidgetNode) -> Option<f32> {
    ["value-label", "label"].into_iter().find_map(|part| {
        base_part_style(&node.style, part)
            .and_then(|style| style.text.font_size)
            .map(|size| size.max(8.0))
    })
}

fn pie_chart_label_text_color(node: &WidgetNode, theme: &Theme) -> Option<[f32; 3]> {
    base_part_style(&node.style, "label").and_then(|style| part_style_text_rgb(style, theme))
}

fn pie_chart_label_font_size(node: &WidgetNode) -> Option<f32> {
    base_part_style(&node.style, "label")
        .and_then(|style| style.text.font_size)
        .map(|size| size.max(8.0))
}

fn emit_bar_chart(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    styled_bg: Option<[f32; 4]>,
    styled_border: Option<[f32; 4]>,
    styled_accent: Option<[f32; 4]>,
    radii: [f32; 4],
    border_w: f32,
) {
    emit_bordered_rect_radii(
        out,
        rect,
        styled_border.unwrap_or(theme.border),
        styled_bg.unwrap_or(theme.surface),
        radii,
        border_w,
    );
    if rect[2] <= 2.0 || rect[3] <= 2.0 {
        return;
    }
    let plot = bar_chart_plot_rect(node, theme, sf, rect);
    let plot_fill = mix(styled_bg.unwrap_or(theme.surface), theme.background, 0.18);
    out.push(inst_radii(plot, plot_fill, [2.0 * sf; 4]));
    let Some(bounds) = bar_chart_resolved_bounds(node) else {
        emit_line_plot_grid(
            out,
            plot,
            theme,
            sf,
            node.props.bar_chart.show_grid,
            node.props.bar_chart.show_axes,
            node.props.bar_chart.show_ticks,
            None,
            &[],
            &[],
        );
        emit_bar_chart_toolbar(out, node, theme, sf, rect);
        return;
    };
    let tick_count = node.props.bar_chart.tick_count.clamp(2, 9);
    if bar_chart_is_horizontal(node) {
        let x_ticks = line_plot_ticks(bounds.x_min, bounds.x_max, tick_count);
        emit_line_plot_grid(
            out,
            plot,
            theme,
            sf,
            node.props.bar_chart.show_grid,
            node.props.bar_chart.show_axes,
            node.props.bar_chart.show_ticks,
            Some(bounds),
            &x_ticks,
            &[],
        );
    } else {
        let y_ticks = line_plot_ticks(bounds.y_min, bounds.y_max, tick_count);
        emit_line_plot_grid(
            out,
            plot,
            theme,
            sf,
            node.props.bar_chart.show_grid,
            node.props.bar_chart.show_axes,
            node.props.bar_chart.show_ticks,
            Some(bounds),
            &[],
            &y_ticks,
        );
    }

    let bars = bar_chart_bar_layouts(node, theme, sf, plot, bounds, styled_accent);
    let hover = node.props.bar_chart.hover.as_ref();
    for bar in &bars {
        let mut color = bar.color;
        if hover
            .is_some_and(|hover| hover.index == bar.index && hover.series_index == bar.series_index)
        {
            color = mix(color, theme.text, 0.14);
        }
        out.push(inst_radii(
            bar.rect,
            color,
            if bar_chart_is_horizontal(node) {
                [0.0, 2.0 * sf, 2.0 * sf, 0.0]
            } else {
                [2.0 * sf, 2.0 * sf, 0.0, 0.0]
            },
        ));
    }
    if let Some(hover) = hover {
        if let Some(bar) = bars
            .iter()
            .find(|bar| bar.index == hover.index && bar.series_index == hover.series_index)
        {
            let mut border = mix(theme.text, theme.accent, 0.18);
            border[3] = 0.94;
            out.push(inst_outline_ring_clipped(
                bar.rect,
                border,
                [2.0 * sf; 4],
                (2.0 * sf).max(1.0),
                [-2.0, -2.0, bar.rect[2] + 4.0, bar.rect[3] + 4.0],
            ));
        }
    }
    emit_bar_chart_toolbar(out, node, theme, sf, rect);
}

fn bar_chart_toolbar_buttons(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
) -> Vec<(&'static str, [f32; 4], bool)> {
    if !bar_chart_toolbar_enabled(node, rect) {
        return Vec::new();
    }
    let pad = 10.0 * sf;
    let button = 24.0 * sf;
    let gap = 5.0 * sf;
    let labels = ["Fit", "Grid", "Axes"];
    let total = button * labels.len() as f32 + gap * (labels.len().saturating_sub(1)) as f32;
    let y = rect[1] + pad;
    let mut x = rect[0] + rect[2] - pad - total;
    let mut buttons = Vec::with_capacity(labels.len());
    for label in labels {
        let active = match label {
            "Grid" => node.props.bar_chart.show_grid,
            "Axes" => node.props.bar_chart.show_axes || node.props.bar_chart.show_ticks,
            _ => true,
        };
        buttons.push((label, [x, y, button, button], active));
        x += button + gap;
    }
    buttons
}

pub(crate) fn bar_chart_toolbar_hit(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
    pos: [f32; 2],
) -> Option<&'static str> {
    for (label, button, _) in bar_chart_toolbar_buttons(node, sf, rect) {
        if pos[0] >= button[0]
            && pos[0] < button[0] + button[2]
            && pos[1] >= button[1]
            && pos[1] < button[1] + button[3]
        {
            return Some(label);
        }
    }
    None
}

fn emit_bar_chart_toolbar(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) {
    for (label, button, active) in bar_chart_toolbar_buttons(node, sf, rect) {
        let mut fill = if active {
            mix(theme.surface_alt, theme.accent, 0.18)
        } else {
            mix(theme.surface_alt, theme.surface, 0.45)
        };
        fill[3] = fill[3].min(0.88);
        let mut border = if active {
            mix(theme.border, theme.accent, 0.50)
        } else {
            mix(theme.border, theme.muted_text, 0.20)
        };
        border[3] = border[3].min(0.68);
        emit_bordered_rect_radii(out, button, border, fill, [4.0 * sf; 4], 1.0 * sf);
        let mut icon = if active {
            mix(theme.text, theme.accent, 0.24)
        } else {
            mix(theme.muted_text, theme.text, 0.20)
        };
        icon[3] = icon[3].min(0.92);
        emit_line_plot_toolbar_icon(out, label, button, icon, sf);
    }
}

pub(crate) fn bar_chart_text_labels(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) -> Vec<LinePlotTextLabel> {
    let mut labels = Vec::new();
    let plot = bar_chart_plot_rect(node, theme, sf, rect);
    let tick_color = mix(theme.muted_text, theme.text, 0.18);
    let tick_color = Some([tick_color[0], tick_color[1], tick_color[2]]);

    if bar_chart_axis_labels_enabled(node, rect) {
        let axis_color = mix(theme.muted_text, theme.text, 0.72);
        let axis_color = Some([axis_color[0], axis_color[1], axis_color[2]]);
        if let Some(label) = node.props.bar_chart.x_label.as_deref() {
            labels.push(LinePlotTextLabel {
                text: label.to_string(),
                screen_x: plot[0] + plot[2] * 0.5,
                screen_y: rect[1] + rect[3] - 11.0 * sf,
                is_title: true,
                anchor: "plot-x-label",
                color: axis_color,
                font_size: Some(LINE_PLOT_AXIS_LABEL_FONT_SIZE_LP),
                clip_rect: None,
            });
        }
        if let Some(label) = node.props.bar_chart.y_label.as_deref() {
            labels.push(LinePlotTextLabel {
                text: label.to_string(),
                screen_x: rect[0] + 18.0 * sf,
                screen_y: plot[1] + plot[3] * 0.5,
                is_title: true,
                anchor: "plot-y-label",
                color: axis_color,
                font_size: Some(LINE_PLOT_AXIS_LABEL_FONT_SIZE_LP),
                clip_rect: None,
            });
        }
    }

    let Some(bounds) = bar_chart_resolved_bounds(node) else {
        return labels;
    };
    if bar_chart_ticks_enabled(node, rect) {
        let tick_count = node.props.bar_chart.tick_count.clamp(2, 9);
        let category_count = node.props.bar_chart.labels.len();
        if bar_chart_is_horizontal(node) {
            let x_ticks = line_plot_ticks(bounds.x_min, bounds.x_max, tick_count);
            let x_step = x_ticks
                .windows(2)
                .next()
                .map(|pair| (pair[1] - pair[0]).abs())
                .unwrap_or_else(|| (bounds.x_max - bounds.x_min).abs());
            for tick in x_ticks {
                let t = ((tick - bounds.x_min) / (bounds.x_max - bounds.x_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                labels.push(LinePlotTextLabel {
                    text: format_line_plot_tick(tick, x_step),
                    screen_x: plot[0] + plot[2] * t,
                    screen_y: plot[1] + plot[3] + 7.0 * sf,
                    is_title: false,
                    anchor: "plot-x-tick",
                    color: tick_color,
                    font_size: Some(10.0),
                    clip_rect: None,
                });
            }
            let group_h = plot[3] / category_count.max(1) as f32;
            if category_count <= 32 && group_h >= 13.0 * sf {
                let label_left = if bar_chart_axis_labels_enabled(node, rect) {
                    rect[0] + 24.0 * sf
                } else {
                    rect[0] + 8.0 * sf
                };
                let label_right = (plot[0] - 5.0 * sf).max(label_left + 1.0);
                for (index, label) in node.props.bar_chart.labels.iter().enumerate() {
                    labels.push(LinePlotTextLabel {
                        text: label.clone(),
                        screen_x: label_right,
                        screen_y: plot[1] + group_h * (index as f32 + 0.5),
                        is_title: false,
                        anchor: "plot-y-category",
                        color: tick_color,
                        font_size: Some(10.0),
                        clip_rect: Some([
                            label_left,
                            plot[1] + group_h * index as f32,
                            label_right - label_left,
                            group_h,
                        ]),
                    });
                }
            }
        } else {
            let y_ticks = line_plot_ticks(bounds.y_min, bounds.y_max, tick_count);
            let y_step = y_ticks
                .windows(2)
                .next()
                .map(|pair| (pair[1] - pair[0]).abs())
                .unwrap_or_else(|| (bounds.y_max - bounds.y_min).abs());
            for tick in y_ticks {
                let t = ((tick - bounds.y_min) / (bounds.y_max - bounds.y_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                labels.push(LinePlotTextLabel {
                    text: format_line_plot_tick(tick, y_step),
                    screen_x: plot[0] - 2.0 * sf,
                    screen_y: plot[1] + plot[3] * (1.0 - t),
                    is_title: false,
                    anchor: "plot-y-tick",
                    color: tick_color,
                    font_size: Some(10.0),
                    clip_rect: None,
                });
            }
            let group_w = plot[2] / category_count.max(1) as f32;
            if category_count <= 32 && group_w >= 24.0 * sf {
                for (index, label) in node.props.bar_chart.labels.iter().enumerate() {
                    labels.push(LinePlotTextLabel {
                        text: label.clone(),
                        screen_x: plot[0] + group_w * (index as f32 + 0.5),
                        screen_y: plot[1] + plot[3] + 7.0 * sf,
                        is_title: false,
                        anchor: "plot-x-tick",
                        color: tick_color,
                        font_size: Some(10.0),
                        clip_rect: Some([
                            plot[0] + group_w * index as f32,
                            plot[1] + plot[3],
                            group_w,
                            rect[1] + rect[3] - plot[1] - plot[3],
                        ]),
                    });
                }
            }
        }
    }

    let bars = bar_chart_bar_layouts(node, theme, sf, plot, bounds, None);
    let value_label_color = bar_chart_value_label_text_color(node, theme);
    let value_label_font_size = bar_chart_value_label_font_size(node).unwrap_or(10.0);
    if bars.len() <= 80 {
        for bar in &bars {
            if bar.rect[2] < 22.0 * sf || bar.rect[3] < 14.0 * sf {
                continue;
            }
            labels.push(LinePlotTextLabel {
                text: format_line_plot_hover_value(bar.value),
                screen_x: bar.center[0],
                screen_y: bar.center[1],
                is_title: false,
                anchor: "box-center",
                color: Some(
                    value_label_color.unwrap_or_else(|| contrast_label_rgb(bar.color, 0.58)),
                ),
                font_size: Some(value_label_font_size),
                clip_rect: Some(bar.rect),
            });
        }
    }

    if let Some(hover) = node.props.bar_chart.hover.as_ref() {
        let series = hover.series_label.as_deref().unwrap_or("value");
        let readout = bar_chart_readout_rect(hover.screen, plot, sf);
        let text_color = mix(theme.text, theme.accent, 0.10);
        labels.push(LinePlotTextLabel {
            text: format!(
                "{}, {series}: {}",
                hover.category,
                format_line_plot_hover_value(hover.value)
            ),
            screen_x: readout[0] + readout[2] * 0.5,
            screen_y: readout[1] + readout[3] * 0.5,
            is_title: false,
            anchor: "plot-readout",
            color: Some([text_color[0], text_color[1], text_color[2]]),
            font_size: Some(10.0),
            clip_rect: Some(readout),
        });
    }

    labels
}

pub(crate) fn heatmap_plot_rect(node: &WidgetNode, sf: f32, rect: [f32; 4]) -> [f32; 4] {
    let base_pad = 10.0 * sf;
    let title_h = if node.props.heatmap.title.is_some() && rect[3] >= 140.0 {
        24.0 * sf
    } else {
        0.0
    };
    let label_x = node.props.heatmap.show_labels
        && !node.props.heatmap.y_labels.is_empty()
        && node.props.heatmap.rows <= 32
        && rect[2] >= 260.0
        && rect[3] >= 180.0;
    let label_y = node.props.heatmap.show_labels
        && !node.props.heatmap.x_labels.is_empty()
        && node.props.heatmap.cols <= 32
        && rect[2] >= 260.0
        && rect[3] >= 180.0;
    let scalar = node.props.heatmap.scalar_bar && rect[2] >= 240.0 && rect[3] >= 150.0;
    let left = if label_x { 58.0 * sf } else { base_pad };
    let bottom = if label_y { 34.0 * sf } else { base_pad };
    let right = if scalar {
        heatmap_scalar_bar_gutter(sf)
    } else {
        base_pad
    };
    let top = base_pad + title_h;
    [
        rect[0] + left,
        rect[1] + top,
        (rect[2] - left - right).max(1.0),
        (rect[3] - top - bottom).max(1.0),
    ]
}

fn heatmap_scalar_bar_gutter(sf: f32) -> f32 {
    (80.0 * sf).max(66.0)
}

fn heatmap_scalar_bar_rect(node: &WidgetNode, sf: f32, rect: [f32; 4]) -> Option<[f32; 4]> {
    if !node.props.heatmap.scalar_bar || rect[2] < 240.0 || rect[3] < 150.0 {
        return None;
    }
    let plot = heatmap_plot_rect(node, sf, rect);
    let width = (12.0 * sf).max(8.0);
    let x = plot[0] + plot[2] + 18.0 * sf;
    Some([x, plot[1], width, plot[3]])
}

fn heatmap_value_color(node: &WidgetNode, value: f32, theme: &Theme) -> [f32; 4] {
    if !value.is_finite() {
        let mut color = mix(theme.surface_alt, theme.background, 0.42);
        color[3] = 0.38;
        return color;
    }
    let span = (node.props.heatmap.vmax - node.props.heatmap.vmin).max(f32::EPSILON);
    let t = ((value - node.props.heatmap.vmin) / span).clamp(0.0, 1.0);
    let rgb = colormap::sample(colormap::resolve(&node.props.heatmap.colormap), t);
    [rgb[0], rgb[1], rgb[2], 1.0]
}

fn heatmap_cell_rect(node: &WidgetNode, plot: [f32; 4], row: usize, col: usize) -> [f32; 4] {
    let rows = node.props.heatmap.rows.max(1) as f32;
    let cols = node.props.heatmap.cols.max(1) as f32;
    let x0 = plot[0] + plot[2] * (col as f32 / cols);
    let x1 = plot[0] + plot[2] * ((col + 1) as f32 / cols);
    let y0 = plot[1] + plot[3] * (row as f32 / rows);
    let y1 = plot[1] + plot[3] * ((row + 1) as f32 / rows);
    [x0, y0, (x1 - x0).max(0.5), (y1 - y0).max(0.5)]
}

fn heatmap_cell_stride(node: &WidgetNode, plot: [f32; 4], sf: f32) -> usize {
    let rows = node.props.heatmap.rows;
    let cols = node.props.heatmap.cols;
    let total = rows.saturating_mul(cols);
    if total <= 4_096 {
        return 1;
    }

    let cell_w = plot[2] / cols.max(1) as f32;
    let cell_h = plot[3] / rows.max(1) as f32;
    if cell_w >= 5.0 * sf && cell_h >= 5.0 * sf {
        return 1;
    }

    let sample_px = (4.0 * sf).max(2.0);
    let screen_target =
        ((plot[2] * plot[3]) / (sample_px * sample_px)).clamp(2_048.0, 12_000.0) as usize;
    ((total as f32 / screen_target.max(1) as f32).sqrt().ceil() as usize).max(2)
}

pub(crate) fn heatmap_cell_at(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
    pos: [f32; 2],
) -> Option<HeatmapHoverProp> {
    let rows = node.props.heatmap.rows;
    let cols = node.props.heatmap.cols;
    if rows == 0 || cols == 0 || node.props.heatmap.values.len() != rows.saturating_mul(cols) {
        return None;
    }
    let plot = heatmap_plot_rect(node, sf, rect);
    if pos[0] < plot[0]
        || pos[0] >= plot[0] + plot[2]
        || pos[1] < plot[1]
        || pos[1] >= plot[1] + plot[3]
    {
        return None;
    }
    let col = (((pos[0] - plot[0]) / plot[2]).clamp(0.0, 0.999_999) * cols as f32) as usize;
    let row = (((pos[1] - plot[1]) / plot[3]).clamp(0.0, 0.999_999) * rows as f32) as usize;
    let value = *node.props.heatmap.values.get(row * cols + col)?;
    let cell = heatmap_cell_rect(node, plot, row, col);
    Some(HeatmapHoverProp {
        row,
        col,
        value,
        screen: [cell[0] + cell[2] * 0.5, cell[1] + cell[3] * 0.5],
        x_label: node.props.heatmap.x_labels.get(col).cloned(),
        y_label: node.props.heatmap.y_labels.get(row).cloned(),
    })
}

fn heatmap_readout_rect(screen: [f32; 2], plot: [f32; 4], sf: f32) -> [f32; 4] {
    let box_w = 186.0 * sf;
    let box_h = 30.0 * sf;
    let mut left = screen[0] + 10.0 * sf;
    let mut top = screen[1] - box_h - 8.0 * sf;
    if left + box_w > plot[0] + plot[2] {
        left = screen[0] - box_w - 10.0 * sf;
    }
    if top < plot[1] {
        top = screen[1] + 10.0 * sf;
    }
    [left, top, box_w, box_h]
}

fn contrast_label_rgb(color: [f32; 4], threshold: f32) -> [f32; 3] {
    let luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
    if luminance > threshold {
        [0.035, 0.045, 0.065]
    } else {
        [0.96, 0.98, 1.0]
    }
}

fn heatmap_cell_text_color(node: &WidgetNode, value: f32, theme: &Theme) -> [f32; 3] {
    contrast_label_rgb(heatmap_value_color(node, value, theme), 0.58)
}

fn emit_heatmap(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    styled_bg: Option<[f32; 4]>,
    styled_border: Option<[f32; 4]>,
    radii: [f32; 4],
    border_w: f32,
) {
    emit_bordered_rect_radii(
        out,
        rect,
        styled_border.unwrap_or(theme.border),
        styled_bg.unwrap_or(theme.surface),
        radii,
        border_w,
    );
    if rect[2] <= 2.0 || rect[3] <= 2.0 {
        return;
    }
    let plot = heatmap_plot_rect(node, sf, rect);
    let plot_fill = mix(styled_bg.unwrap_or(theme.surface), theme.background, 0.18);
    out.push(inst_radii(plot, plot_fill, [2.0 * sf; 4]));

    let rows = node.props.heatmap.rows;
    let cols = node.props.heatmap.cols;
    if rows == 0 || cols == 0 || node.props.heatmap.values.len() != rows.saturating_mul(cols) {
        return;
    }
    let grid_visual = base_part_style(&node.style, "grid")
        .map(|style| style.visual.clone())
        .unwrap_or_default();
    let cell_visual = base_part_style(&node.style, "cell")
        .map(|style| style.visual.clone())
        .unwrap_or_default();
    let cell_fill = resolve_color(&cell_visual.background, theme)
        .or_else(|| resolve_color(&cell_visual.foreground, theme))
        .map(|color| apply_opacity(color, cell_visual.opacity));
    let grid_fallback = native_widget_part_paint_fallback(
        WidgetKind::Heatmap,
        "grid",
        theme,
        PaintInteraction::Resting,
        false,
    );
    let scalar_bar_visual = base_part_style(&node.style, "scalar-bar")
        .map(|style| style.visual.clone())
        .unwrap_or_default();
    let scalar_bar_fallback = native_widget_part_paint_fallback(
        WidgetKind::Heatmap,
        "scalar-bar",
        theme,
        PaintInteraction::Resting,
        false,
    );

    let stride = heatmap_cell_stride(node, plot, sf);
    for row in (0..rows).step_by(stride) {
        let row_end = (row + stride).min(rows);
        let sample_row = row + (row_end - row) / 2;
        for col in (0..cols).step_by(stride) {
            let col_end = (col + stride).min(cols);
            let sample_col = col + (col_end - col) / 2;
            let Some(value) = node
                .props
                .heatmap
                .values
                .get(sample_row * cols + sample_col)
                .copied()
            else {
                continue;
            };
            let x0 = plot[0] + plot[2] * (col as f32 / cols as f32);
            let x1 = plot[0] + plot[2] * (col_end as f32 / cols as f32);
            let y0 = plot[1] + plot[3] * (row as f32 / rows as f32);
            let y1 = plot[1] + plot[3] * (row_end as f32 / rows as f32);
            out.push(inst_radii(
                [x0, y0, (x1 - x0).max(0.5), (y1 - y0).max(0.5)],
                cell_fill.unwrap_or_else(|| heatmap_value_color(node, value, theme)),
                [0.0; 4],
            ));
        }
    }

    let cell_w = plot[2] / cols.max(1) as f32;
    let cell_h = plot[3] / rows.max(1) as f32;
    if rows <= 40 && cols <= 40 && cell_w >= 6.0 * sf && cell_h >= 6.0 * sf {
        let grid_color = resolve_color(&grid_visual.background, theme)
            .map(|color| apply_opacity(color, grid_visual.opacity))
            .or(grid_fallback.background)
            .unwrap_or_else(|| with_alpha(mix(theme.border, theme.background, 0.16), 0.38));
        let grid_w = (grid_visual
            .border_width
            .or(grid_fallback.border_width)
            .unwrap_or(1.0)
            * sf)
            .max(1.0);
        for col in 1..cols {
            let x = plot[0] + plot[2] * (col as f32 / cols as f32);
            out.push(inst_radii(
                [x, plot[1], grid_w, plot[3]],
                grid_color,
                [0.0; 4],
            ));
        }
        for row in 1..rows {
            let y = plot[1] + plot[3] * (row as f32 / rows as f32);
            out.push(inst_radii(
                [plot[0], y, plot[2], grid_w],
                grid_color,
                [0.0; 4],
            ));
        }
    }

    if let Some(bar) = heatmap_scalar_bar_rect(node, sf, rect) {
        if let Some(fill) = resolve_color(&scalar_bar_visual.background, theme)
            .map(|color| apply_opacity(color, scalar_bar_visual.opacity))
        {
            out.push(inst_radii(bar, fill, [0.0; 4]));
        } else {
            let steps = 64usize;
            let cmap = colormap::resolve(&node.props.heatmap.colormap);
            for index in 0..steps {
                let t0 = index as f32 / steps as f32;
                let t1 = (index + 1) as f32 / steps as f32;
                let rgb = colormap::sample(cmap, 1.0 - (t0 + t1) * 0.5);
                out.push(inst_radii(
                    [
                        bar[0],
                        bar[1] + bar[3] * t0,
                        bar[2],
                        (bar[3] * (t1 - t0)).max(0.75),
                    ],
                    [rgb[0], rgb[1], rgb[2], 1.0],
                    [0.0; 4],
                ));
            }
        }
        let scalar_bar_radius = scalar_bar_visual
            .border_radius
            .or(scalar_bar_fallback.border_radius)
            .unwrap_or(2.0)
            .max(0.0)
            * sf;
        let scalar_bar_border_width = scalar_bar_visual
            .border_width
            .or(scalar_bar_fallback.border_width)
            .unwrap_or(1.0)
            .max(0.0)
            * sf;
        out.push(inst_outline_ring_clipped(
            bar,
            resolve_color(&scalar_bar_visual.border_color, theme)
                .map(|color| apply_opacity(color, scalar_bar_visual.opacity))
                .or(scalar_bar_fallback.border_color)
                .or(styled_border)
                .unwrap_or(theme.border),
            [scalar_bar_radius; 4],
            scalar_bar_border_width,
            [-1.0, -1.0, bar[2] + 2.0, bar[3] + 2.0],
        ));
    }

    if let Some(hover) = node.props.heatmap.hover.as_ref() {
        if hover.row < rows && hover.col < cols {
            let cell = heatmap_cell_rect(node, plot, hover.row, hover.col);
            let hover_visual = base_part_style(&node.style, "hover")
                .map(|style| style.visual.clone())
                .unwrap_or_default();
            let hover_fallback = native_widget_part_paint_fallback(
                WidgetKind::Heatmap,
                "hover",
                theme,
                PaintInteraction::Hovered,
                false,
            );
            let fill = resolve_color(&hover_visual.background, theme)
                .map(|color| apply_opacity(color, hover_visual.opacity))
                .or(hover_fallback.background)
                .unwrap_or_else(|| with_alpha(mix(theme.accent, theme.surface, 0.26), 0.14));
            let border = resolve_color(&hover_visual.border_color, theme)
                .map(|color| apply_opacity(color, hover_visual.opacity))
                .or(hover_fallback.border_color)
                .unwrap_or_else(|| with_alpha(mix(theme.text, theme.accent, 0.20), 0.94));
            let hover_radius = hover_visual
                .border_radius
                .or(hover_fallback.border_radius)
                .unwrap_or(1.5)
                .max(0.0)
                * sf;
            let hover_border_width = hover_visual
                .border_width
                .or(hover_fallback.border_width)
                .unwrap_or(2.0)
                .max(0.0)
                * sf;
            emit_bordered_rect_radii(
                out,
                cell,
                border,
                fill,
                [hover_radius; 4],
                hover_border_width,
            );
            let readout = heatmap_readout_rect(hover.screen, plot, sf);
            let mut bg = mix(theme.surface, theme.background, 0.12);
            bg[3] = 0.98;
            let mut readout_border = mix(theme.border, theme.accent, 0.42);
            readout_border[3] = 0.82;
            emit_bordered_rect_radii(
                out,
                readout,
                readout_border,
                bg,
                [5.0 * sf; 4],
                (1.0 * sf).max(1.0),
            );
        }
    }
}

pub(crate) fn heatmap_text_labels(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) -> Vec<LinePlotTextLabel> {
    let mut labels = Vec::new();
    let plot = heatmap_plot_rect(node, sf, rect);
    let color = mix(theme.muted_text, theme.text, 0.56);
    let color = Some([color[0], color[1], color[2]]);

    if let Some(title) = node.props.heatmap.title.as_deref() {
        labels.push(LinePlotTextLabel {
            text: title.to_string(),
            screen_x: rect[0] + 12.0 * sf,
            screen_y: rect[1] + 8.0 * sf,
            is_title: true,
            anchor: "top-left",
            color,
            font_size: Some(13.0),
            clip_rect: Some(rect),
        });
    }

    if node.props.heatmap.show_labels {
        let rows = node.props.heatmap.rows.max(1);
        let cols = node.props.heatmap.cols.max(1);
        let cell_w = plot[2] / cols as f32;
        let cell_h = plot[3] / rows as f32;
        if !node.props.heatmap.x_labels.is_empty() && cols <= 32 && cell_w >= 18.0 * sf {
            for (index, label) in node.props.heatmap.x_labels.iter().enumerate() {
                labels.push(LinePlotTextLabel {
                    text: label.clone(),
                    screen_x: plot[0] + cell_w * (index as f32 + 0.5),
                    screen_y: plot[1] + plot[3] + 6.0 * sf,
                    is_title: false,
                    anchor: "plot-x-tick",
                    color,
                    font_size: Some(10.0),
                    clip_rect: Some([
                        plot[0] + cell_w * index as f32,
                        plot[1] + plot[3],
                        cell_w,
                        rect[1] + rect[3] - plot[1] - plot[3],
                    ]),
                });
            }
        }
        if !node.props.heatmap.y_labels.is_empty() && rows <= 32 && cell_h >= 13.0 * sf {
            for (index, label) in node.props.heatmap.y_labels.iter().enumerate() {
                labels.push(LinePlotTextLabel {
                    text: label.clone(),
                    screen_x: plot[0] - 4.0 * sf,
                    screen_y: plot[1] + cell_h * (index as f32 + 0.5),
                    is_title: false,
                    anchor: "plot-y-tick",
                    color,
                    font_size: Some(10.0),
                    clip_rect: Some([rect[0], plot[1], plot[0] - rect[0], plot[3]]),
                });
            }
        }
        if node
            .props
            .heatmap
            .rows
            .saturating_mul(node.props.heatmap.cols)
            <= 100
            && cell_w >= 38.0 * sf
            && cell_h >= 22.0 * sf
        {
            for row in 0..node.props.heatmap.rows {
                for col in 0..node.props.heatmap.cols {
                    let Some(value) = node
                        .props
                        .heatmap
                        .values
                        .get(row * node.props.heatmap.cols + col)
                    else {
                        continue;
                    };
                    if !value.is_finite() {
                        continue;
                    }
                    let cell = heatmap_cell_rect(node, plot, row, col);
                    let rgb = heatmap_cell_text_color(node, *value, theme);
                    labels.push(LinePlotTextLabel {
                        text: format_line_plot_hover_value(*value),
                        screen_x: cell[0] + cell[2] * 0.5,
                        screen_y: cell[1] + cell[3] * 0.5,
                        is_title: false,
                        anchor: "box-center",
                        color: Some(rgb),
                        font_size: Some(10.0),
                        clip_rect: Some(cell),
                    });
                }
            }
        }
    }

    if let Some(bar) = heatmap_scalar_bar_rect(node, sf, rect) {
        labels.push(LinePlotTextLabel {
            text: format_line_plot_hover_value(node.props.heatmap.vmax),
            screen_x: bar[0] + bar[2] + 6.0 * sf,
            screen_y: bar[1] - 1.0 * sf,
            is_title: false,
            anchor: "top-left",
            color,
            font_size: Some(10.0),
            clip_rect: Some(rect),
        });
        labels.push(LinePlotTextLabel {
            text: format_line_plot_hover_value(node.props.heatmap.vmin),
            screen_x: bar[0] + bar[2] + 6.0 * sf,
            screen_y: bar[1] + bar[3] - 12.0 * sf,
            is_title: false,
            anchor: "top-left",
            color,
            font_size: Some(10.0),
            clip_rect: Some(rect),
        });
    }

    if let Some(hover) = node.props.heatmap.hover.as_ref() {
        let row = hover
            .y_label
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| format!("row {}", hover.row));
        let col = hover
            .x_label
            .as_deref()
            .map(str::to_string)
            .unwrap_or_else(|| format!("col {}", hover.col));
        let readout = heatmap_readout_rect(hover.screen, plot, sf);
        let text_color = mix(theme.text, theme.accent, 0.10);
        labels.push(LinePlotTextLabel {
            text: format!(
                "{row}, {col}: {}",
                format_line_plot_hover_value(hover.value)
            ),
            screen_x: readout[0] + readout[2] * 0.5,
            screen_y: readout[1] + readout[3] * 0.5,
            is_title: false,
            anchor: "plot-readout",
            color: Some([text_color[0], text_color[1], text_color[2]]),
            font_size: Some(10.0),
            clip_rect: Some(readout),
        });
    }

    labels
}

pub(crate) fn line_plot_resolved_bounds(node: &WidgetNode) -> Option<LinePlotBounds> {
    if !node.props.line_plot_auto_fit {
        if let (Some(x_min), Some(x_max), Some(y_min), Some(y_max)) = (
            node.props.line_plot_x_min,
            node.props.line_plot_x_max,
            node.props.line_plot_y_min,
            node.props.line_plot_y_max,
        ) {
            if x_min.is_finite()
                && x_max.is_finite()
                && y_min.is_finite()
                && y_max.is_finite()
                && x_max > x_min
                && y_max > y_min
            {
                return Some(LinePlotBounds {
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                });
            }
        }
    }
    line_plot_data_bounds(node).map(expand_line_plot_bounds)
}

fn line_plot_data_bounds(node: &WidgetNode) -> Option<LinePlotBounds> {
    let mut bounds = LinePlotBounds {
        x_min: f32::INFINITY,
        x_max: f32::NEG_INFINITY,
        y_min: f32::INFINITY,
        y_max: f32::NEG_INFINITY,
    };
    let mut has_point = false;
    for series in &node.props.line_plot_series {
        if let Some([x_min, x_max, y_min, y_max]) = series.bounds {
            if x_min.is_finite() && x_max.is_finite() && y_min.is_finite() && y_max.is_finite() {
                bounds.x_min = bounds.x_min.min(x_min);
                bounds.x_max = bounds.x_max.max(x_max);
                bounds.y_min = bounds.y_min.min(y_min);
                bounds.y_max = bounds.y_max.max(y_max);
                has_point = true;
                continue;
            }
        }
        for [px, py] in series.logical_points() {
            if px.is_finite() && py.is_finite() {
                bounds.x_min = bounds.x_min.min(*px);
                bounds.x_max = bounds.x_max.max(*px);
                bounds.y_min = bounds.y_min.min(*py);
                bounds.y_max = bounds.y_max.max(*py);
                has_point = true;
            }
        }
    }
    has_point.then_some(bounds)
}

fn line_plot_visible_point_bounds(points: &[[f32; 2]], bounds: LinePlotBounds) -> (usize, usize) {
    if points.is_empty() {
        return (0, 0);
    }
    let first = points.first().map(|point| point[0]).unwrap_or(0.0);
    let last = points.last().map(|point| point[0]).unwrap_or(0.0);
    if !first.is_finite() || !last.is_finite() || first > last {
        return (0, points.len());
    }
    let start = points.partition_point(|point| point[0] < bounds.x_min);
    let end = points.partition_point(|point| point[0] <= bounds.x_max);
    if start < end {
        return (start.saturating_sub(1), (end + 1).min(points.len()));
    }
    if start > 0 && start < points.len() {
        return (start - 1, start + 1);
    }
    (start, end)
}

fn expand_line_plot_bounds(mut bounds: LinePlotBounds) -> LinePlotBounds {
    if (bounds.x_max - bounds.x_min).abs() <= f32::EPSILON {
        let pad = bounds.x_min.abs().max(1.0) * 0.5;
        bounds.x_min -= pad;
        bounds.x_max += pad;
    }
    if (bounds.y_max - bounds.y_min).abs() <= f32::EPSILON {
        let pad = bounds.y_min.abs().max(1.0) * 0.5;
        bounds.y_min -= pad;
        bounds.y_max += pad;
    } else {
        let pad = (bounds.y_max - bounds.y_min).abs() * 0.04;
        bounds.y_min -= pad;
        bounds.y_max += pad;
    }
    bounds
}

fn line_plot_outer_padding(node: &WidgetNode, sf: f32, rect: [f32; 4]) -> f32 {
    let pad_lp = uniform_layout_padding(&node.style.layout).unwrap_or(12.0);
    (pad_lp.max(4.0) * sf).min(rect[2].min(rect[3]) * 0.22)
}

fn line_plot_toolbar_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.line_plot_show_toolbar && rect[2] >= 260.0 && rect[3] >= 180.0
}

fn line_plot_ticks_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.line_plot_show_ticks && rect[2] >= 240.0 && rect[3] >= 170.0
}

fn line_plot_axis_labels_enabled(node: &WidgetNode, rect: [f32; 4]) -> bool {
    node.props.line_plot_show_axes && rect[2] >= 260.0 && rect[3] >= 220.0
}

fn line_plot_y_axis_label(node: &WidgetNode) -> Option<&str> {
    node.props.line_plot_y_label.as_deref().or_else(|| {
        node.props
            .line_plot_series
            .iter()
            .find_map(|series| series.label.as_deref())
    })
}

pub(crate) fn line_plot_plot_rect(node: &WidgetNode, sf: f32, rect: [f32; 4]) -> [f32; 4] {
    let [x, y, w, h] = rect;
    let pad = line_plot_outer_padding(node, sf, rect);
    let show_ticks = line_plot_ticks_enabled(node, rect);
    let show_axis_labels = line_plot_axis_labels_enabled(node, rect);
    let show_toolbar = line_plot_toolbar_enabled(node, rect);
    let left_extra = if show_ticks { 30.0 * sf } else { 0.0 };
    let bottom_extra = if show_ticks { 24.0 * sf } else { 0.0 };
    let left_label_extra = if show_axis_labels {
        LINE_PLOT_AXIS_LABEL_GUTTER_LP * sf
    } else {
        0.0
    };
    let bottom_label_extra = if show_axis_labels { 18.0 * sf } else { 0.0 };
    let top_extra = if show_toolbar { 30.0 * sf } else { 0.0 };
    let right_extra = if show_ticks { 8.0 * sf } else { 0.0 };
    let left = (pad + left_extra + left_label_extra).min(w * 0.42);
    let right = (pad + right_extra).min(w * 0.24);
    let top = (pad + top_extra).min(h * 0.36);
    let bottom = (pad + bottom_extra + bottom_label_extra).min(h * 0.38);
    [
        x + left,
        y + top,
        (w - left - right).max(1.0),
        (h - top - bottom).max(1.0),
    ]
}

fn line_plot_ticks(min: f32, max: f32, target_count: usize) -> Vec<f32> {
    if !min.is_finite() || !max.is_finite() || max <= min {
        return Vec::new();
    }
    let target = target_count.clamp(2, 9) as f32;
    let range = max - min;
    let step = nice_line_plot_step(range / (target - 1.0));
    if !step.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    let start = (min / step).ceil() * step;
    let mut value = start;
    let mut ticks = Vec::new();
    while value <= max + step * 0.5 && ticks.len() < 12 {
        if value >= min - step * 0.5 {
            ticks.push(if value.abs() < step * 1.0e-4 {
                0.0
            } else {
                value
            });
        }
        value += step;
    }
    ticks
}

fn nice_line_plot_step(raw_step: f32) -> f32 {
    if !raw_step.is_finite() || raw_step <= 0.0 {
        return 1.0;
    }
    let exponent = raw_step.log10().floor();
    let base = 10.0_f32.powf(exponent);
    let fraction = raw_step / base;
    let nice_fraction = if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice_fraction * base
}

fn line_plot_tick_decimals(step: f32) -> usize {
    if !step.is_finite() || step <= 0.0 || step >= 1.0 {
        return 0;
    }
    (-step.log10().floor() as usize + 1).min(5)
}

fn format_line_plot_tick(value: f32, step: f32) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let abs = value.abs();
    if abs >= 10_000.0 || (abs > 0.0 && abs < 0.001) {
        return format!("{value:.1e}");
    }
    let decimals = line_plot_tick_decimals(step);
    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        "0".to_string()
    } else {
        text
    }
}

fn emit_line_plot_grid(
    out: &mut Vec<RectInstance>,
    plot: [f32; 4],
    theme: &Theme,
    sf: f32,
    show_grid: bool,
    show_axes: bool,
    show_ticks: bool,
    bounds: Option<LinePlotBounds>,
    x_ticks: &[f32],
    y_ticks: &[f32],
) {
    let stroke = (1.0 * sf).max(1.0);
    let mut axis_color = mix(theme.border, theme.text, 0.18);
    axis_color[3] = axis_color[3].min(0.72);
    if show_grid {
        let mut grid_color = mix(theme.border, theme.muted_text, 0.18);
        grid_color[3] = grid_color[3].min(0.34);
        if let Some(bounds) = bounds {
            for tick in x_ticks {
                let t = ((*tick - bounds.x_min) / (bounds.x_max - bounds.x_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                if t <= 0.001 || t >= 0.999 {
                    continue;
                }
                let gx = plot[0] + plot[2] * t;
                out.push(inst(
                    [gx - stroke * 0.5, plot[1], stroke, plot[3]],
                    grid_color,
                    0.0,
                ));
            }
            for tick in y_ticks {
                let t = ((*tick - bounds.y_min) / (bounds.y_max - bounds.y_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                if t <= 0.001 || t >= 0.999 {
                    continue;
                }
                let gy = plot[1] + plot[3] * (1.0 - t);
                out.push(inst(
                    [plot[0], gy - stroke * 0.5, plot[2], stroke],
                    grid_color,
                    0.0,
                ));
            }
        } else {
            for i in 1..4 {
                let t = i as f32 / 4.0;
                let gx = plot[0] + plot[2] * t;
                let gy = plot[1] + plot[3] * t;
                out.push(inst(
                    [gx - stroke * 0.5, plot[1], stroke, plot[3]],
                    grid_color,
                    0.0,
                ));
                out.push(inst(
                    [plot[0], gy - stroke * 0.5, plot[2], stroke],
                    grid_color,
                    0.0,
                ));
            }
        }
    }
    if show_axes {
        if let Some(bounds) = bounds {
            let mut zero_color = mix(theme.border, theme.text, 0.30);
            zero_color[3] = zero_color[3].min(0.46);
            if bounds.y_min < 0.0 && bounds.y_max > 0.0 {
                let t = ((0.0 - bounds.y_min) / (bounds.y_max - bounds.y_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                let gy = plot[1] + plot[3] * (1.0 - t);
                out.push(inst(
                    [plot[0], gy - stroke * 0.5, plot[2], stroke],
                    zero_color,
                    0.0,
                ));
            }
            if bounds.x_min < 0.0 && bounds.x_max > 0.0 {
                let t = ((0.0 - bounds.x_min) / (bounds.x_max - bounds.x_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                let gx = plot[0] + plot[2] * t;
                out.push(inst(
                    [gx - stroke * 0.5, plot[1], stroke, plot[3]],
                    zero_color,
                    0.0,
                ));
            }
        }
    }
    if show_axes {
        out.push(inst(
            [plot[0], plot[1] + plot[3] - stroke, plot[2], stroke],
            axis_color,
            0.0,
        ));
        out.push(inst([plot[0], plot[1], stroke, plot[3]], axis_color, 0.0));
    }
    if show_ticks {
        let tick_len = 4.0 * sf;
        if let Some(bounds) = bounds {
            for tick in x_ticks {
                let t = ((*tick - bounds.x_min) / (bounds.x_max - bounds.x_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                let gx = plot[0] + plot[2] * t;
                out.push(inst(
                    [gx - stroke * 0.5, plot[1] + plot[3], stroke, tick_len],
                    axis_color,
                    0.0,
                ));
            }
            for tick in y_ticks {
                let t = ((*tick - bounds.y_min) / (bounds.y_max - bounds.y_min).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                let gy = plot[1] + plot[3] * (1.0 - t);
                out.push(inst(
                    [plot[0] - tick_len, gy - stroke * 0.5, tick_len, stroke],
                    axis_color,
                    0.0,
                ));
            }
        }
    }
}

fn line_plot_toolbar_buttons(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
) -> Vec<(&'static str, [f32; 4], bool)> {
    if !line_plot_toolbar_enabled(node, rect) {
        return Vec::new();
    }
    let pad = line_plot_outer_padding(node, sf, rect);
    let button = 24.0 * sf;
    let gap = 5.0 * sf;
    let total = button * 6.0 + gap * 5.0;
    let y = rect[1] + pad;
    let mut x = rect[0] + rect[2] - pad - total;
    let mut buttons = Vec::with_capacity(6);
    for label in ["Fit", "Pan", "Zoom", "Box", "Grid", "Axes"] {
        let active = match label {
            "Pan" => node.props.line_plot_interaction == "pan",
            "Zoom" => node.props.line_plot_interaction == "zoom",
            "Box" => node.props.line_plot_interaction == "box_zoom",
            "Grid" => node.props.line_plot_show_grid,
            "Axes" => node.props.line_plot_show_axes || node.props.line_plot_show_ticks,
            _ => true,
        };
        buttons.push((label, [x, y, button, button], active));
        x += button + gap;
    }
    buttons
}

pub(crate) fn line_plot_toolbar_hit(
    node: &WidgetNode,
    sf: f32,
    rect: [f32; 4],
    pos: [f32; 2],
) -> Option<&'static str> {
    for (label, button, _) in line_plot_toolbar_buttons(node, sf, rect) {
        if pos[0] >= button[0]
            && pos[0] < button[0] + button[2]
            && pos[1] >= button[1]
            && pos[1] < button[1] + button[3]
        {
            return Some(label);
        }
    }
    None
}

fn emit_line_plot_toolbar(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) {
    for (label, button, active) in line_plot_toolbar_buttons(node, sf, rect) {
        let mut fill = if active {
            mix(theme.surface_alt, theme.accent, 0.18)
        } else {
            mix(theme.surface_alt, theme.surface, 0.45)
        };
        fill[3] = fill[3].min(0.88);
        let mut border = if active {
            mix(theme.border, theme.accent, 0.50)
        } else {
            mix(theme.border, theme.muted_text, 0.20)
        };
        border[3] = border[3].min(0.68);
        emit_bordered_rect_radii(out, button, border, fill, [4.0 * sf; 4], 1.0 * sf);
        let mut icon = if active {
            mix(theme.text, theme.accent, 0.24)
        } else {
            mix(theme.muted_text, theme.text, 0.20)
        };
        icon[3] = icon[3].min(0.92);
        emit_line_plot_toolbar_icon(out, label, button, icon, sf);
    }
}

fn emit_line_plot_toolbar_icon(
    out: &mut Vec<RectInstance>,
    label: &str,
    rect: [f32; 4],
    color: Color,
    sf: f32,
) {
    match label {
        "Fit" => emit_line_plot_fit_icon(out, rect, color, sf),
        "Pan" => emit_line_plot_pan_icon(out, rect, color, sf),
        "Zoom" => emit_line_plot_zoom_icon(out, rect, color, sf),
        "Box" => emit_line_plot_box_zoom_icon(out, rect, color, sf),
        "Grid" => emit_line_plot_grid_icon(out, rect, color, sf),
        "Axes" => emit_line_plot_axes_icon(out, rect, color, sf),
        _ => {}
    }
}

fn emit_line_plot_selection_rect(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    plot: [f32; 4],
) {
    let Some(raw) = node.props.line_plot_selection_rect else {
        return;
    };
    let x0 = raw[0].min(raw[2]).clamp(plot[0], plot[0] + plot[2]);
    let x1 = raw[0].max(raw[2]).clamp(plot[0], plot[0] + plot[2]);
    let y0 = raw[1].min(raw[3]).clamp(plot[1], plot[1] + plot[3]);
    let y1 = raw[1].max(raw[3]).clamp(plot[1], plot[1] + plot[3]);
    let rect = [x0, y0, x1 - x0, y1 - y0];
    if rect[2] < 2.0 * sf || rect[3] < 2.0 * sf {
        return;
    }
    let mut fill = mix(theme.accent, theme.surface, 0.24);
    fill[3] = 0.18;
    let mut border = mix(theme.accent, theme.text, 0.20);
    border[3] = 0.82;
    emit_bordered_rect_radii(out, rect, border, fill, [2.0 * sf; 4], 1.0 * sf);
}

fn emit_line_plot_hover(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    plot: [f32; 4],
) {
    let Some(hover) = node.props.line_plot_hover.as_ref() else {
        return;
    };
    let sx = hover.screen[0];
    let sy = hover.screen[1];
    if sx < plot[0] || sx > plot[0] + plot[2] || sy < plot[1] || sy > plot[1] + plot[3] {
        return;
    }
    let mut cross = mix(theme.muted_text, theme.accent, 0.44);
    cross[3] = 0.46;
    let stroke = (1.0 * sf).max(1.0);
    out.push(inst(
        [sx - stroke * 0.5, plot[1], stroke, plot[3]],
        cross,
        0.0,
    ));
    out.push(inst(
        [plot[0], sy - stroke * 0.5, plot[2], stroke],
        cross,
        0.0,
    ));

    let point_color = hover
        .color
        .as_ref()
        .map(|color| color.resolve(theme))
        .unwrap_or(theme.accent);
    let marker = (6.0 * sf).max(5.0);
    emit_bordered_rect_radii(
        out,
        [sx - marker * 0.5, sy - marker * 0.5, marker, marker],
        theme.surface,
        point_color,
        [marker * 0.5; 4],
        (1.25 * sf).max(1.0),
    );

    let readout = line_plot_hover_readout_rect(hover.screen, plot, sf);
    let mut fill = mix(theme.surface, theme.background, 0.22);
    fill[3] = 0.94;
    let mut border = mix(theme.border, theme.accent, 0.42);
    border[3] = 0.80;
    emit_bordered_rect_radii(
        out,
        readout,
        border,
        fill,
        [5.0 * sf; 4],
        (1.0 * sf).max(1.0),
    );
}

fn line_plot_hover_readout_rect(screen: [f32; 2], plot: [f32; 4], sf: f32) -> [f32; 4] {
    let box_w = 168.0 * sf;
    let box_h = 24.0 * sf;
    let mut left = screen[0] + 10.0 * sf;
    let mut top = screen[1] - box_h - 8.0 * sf;
    if left + box_w > plot[0] + plot[2] {
        left = screen[0] - box_w - 10.0 * sf;
    }
    if top < plot[1] {
        top = screen[1] + 10.0 * sf;
    }
    [left, top, box_w, box_h]
}

fn line_plot_legend_entries(node: &WidgetNode, theme: &Theme) -> Vec<(String, [f32; 4], String)> {
    node.props
        .line_plot_series
        .iter()
        .enumerate()
        .filter_map(|(index, series)| {
            let label = series
                .label
                .as_deref()
                .filter(|label| !label.trim().is_empty())?;
            let color = series
                .color
                .as_ref()
                .map(|color| color.resolve(theme))
                .unwrap_or(LINE_PLOT_PALETTE[index % LINE_PLOT_PALETTE.len()]);
            Some((label.to_string(), color, series.line_style.clone()))
        })
        .collect()
}

fn line_plot_legend_rect(
    node: &WidgetNode,
    theme: &Theme,
    plot: [f32; 4],
    sf: f32,
) -> Option<[f32; 4]> {
    if !node.props.line_plot_show_legend {
        return None;
    }
    let labels = node
        .props
        .line_plot_series
        .iter()
        .filter_map(|series| {
            series.label.as_deref().and_then(|label| {
                let label = label.trim();
                (!label.is_empty()).then_some(label)
            })
        })
        .collect::<Vec<_>>();
    let entries = labels.len();
    if entries == 0 {
        return None;
    }
    let pad = 8.0 * sf;
    let text_style = TextStyle {
        font_size: Some(10.0),
        ..node.style.text.clone()
    };
    let longest = labels
        .iter()
        .map(|label| measure_text_for_layout(label, &text_style, theme).width)
        .fold(0.0, f32::max);
    let label_w = (longest * sf).clamp(22.0 * sf, 86.0 * sf);
    let w = (37.0 * sf + label_w).min((plot[2] - pad * 2.0).max(50.0 * sf));
    let h = (entries as f32 * 17.0 * sf + 8.0 * sf).min((plot[3] - pad * 2.0).max(26.0 * sf));
    let x = match node.props.line_plot_legend_position.as_str() {
        "top-left" | "bottom-left" => plot[0] + pad,
        _ => plot[0] + plot[2] - pad - w,
    };
    let y = match node.props.line_plot_legend_position.as_str() {
        "bottom-left" | "bottom-right" => plot[1] + plot[3] - pad - h,
        _ => plot[1] + pad,
    };
    Some([x, y, w, h])
}

fn emit_line_plot_legend(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    plot: [f32; 4],
) {
    let Some(rect) = line_plot_legend_rect(node, theme, plot, sf) else {
        return;
    };
    let entries = line_plot_legend_entries(node, theme);
    if entries.is_empty() {
        return;
    }
    let mut fill = mix(theme.surface, theme.background, 0.18);
    fill[3] = 0.92;
    let mut border = mix(theme.border, theme.accent, 0.28);
    border[3] = 0.62;
    emit_bordered_rect_radii(out, rect, border, fill, [6.0 * sf; 4], (1.0 * sf).max(1.0));

    let x0 = rect[0] + 7.0 * sf;
    let row_h = 17.0 * sf;
    let content_h = entries.len() as f32 * row_h;
    let mut cy = rect[1] + (rect[3] - content_h).max(0.0) * 0.5 + row_h * 0.5;
    for (_, color, style) in entries {
        push_styled_line_segment(
            out,
            [x0, cy],
            [x0 + 22.0 * sf, cy],
            (2.0 * sf).max(1.0),
            color,
            &style,
        );
        cy += row_h;
    }
}

fn emit_line_plot_box_zoom_icon(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    sf: f32,
) {
    let [x, y, w, h] = rect;
    let stroke = (1.3 * sf).max(1.0);
    let radius = stroke * 0.5;
    let box_w = (12.0 * sf).min(w.min(h) * 0.56).max(stroke * 6.0);
    let left = x + (w - box_w) * 0.5;
    let top = y + (h - box_w) * 0.5;
    let dash = box_w * 0.34;
    for (rx, ry, sx, sy) in [
        (left, top, 1.0, 1.0),
        (left + box_w, top, -1.0, 1.0),
        (left + box_w, top + box_w, -1.0, -1.0),
        (left, top + box_w, 1.0, -1.0),
    ] {
        out.push(inst_radii(
            [
                if sx > 0.0 { rx } else { rx - dash },
                ry - stroke * 0.5,
                dash,
                stroke,
            ],
            color,
            [radius; 4],
        ));
        out.push(inst_radii(
            [
                rx - stroke * 0.5,
                if sy > 0.0 { ry } else { ry - dash },
                stroke,
                dash,
            ],
            color,
            [radius; 4],
        ));
    }
}

fn emit_line_plot_pan_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.35 * sf).max(1.0);
    let radius = stroke * 0.5;
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let arm = (5.2 * sf).min(w.min(h) * 0.24).max(stroke * 2.0);
    out.push(inst_radii(
        [cx - arm, cy - stroke * 0.5, arm * 2.0, stroke],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [cx - stroke * 0.5, cy - arm, stroke, arm * 2.0],
        color,
        [radius; 4],
    ));
    let head = (3.1 * sf).max(stroke * 1.5);
    for (hx, hy, horizontal) in [
        (cx - arm - head * 0.35, cy, true),
        (cx + arm - head * 0.65, cy, true),
        (cx, cy - arm - head * 0.35, false),
        (cx, cy + arm - head * 0.65, false),
    ] {
        if horizontal {
            out.push(inst_radii(
                [hx, hy - head * 0.5, head, head],
                color,
                [head * 0.5; 4],
            ));
        } else {
            out.push(inst_radii(
                [hx - head * 0.5, hy, head, head],
                color,
                [head * 0.5; 4],
            ));
        }
    }
}

fn emit_line_plot_zoom_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.35 * sf).max(1.0);
    let radius = stroke * 0.5;
    let lens = (8.8 * sf).min(w.min(h) * 0.42).max(stroke * 4.0);
    let left = x + w * 0.5 - lens * 0.64;
    let top = y + h * 0.5 - lens * 0.64;
    let right = left + lens;
    let bottom = top + lens;
    let third = lens / 3.0;
    out.push(inst_radii(
        [left + third * 0.42, top, third * 2.16, stroke],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [left + third * 0.42, bottom - stroke, third * 2.16, stroke],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [left, top + third * 0.42, stroke, third * 2.16],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [right - stroke, top + third * 0.42, stroke, third * 2.16],
        color,
        [radius; 4],
    ));
    let handle_len = (6.6 * sf).min(w.min(h) * 0.30).max(stroke * 3.2);
    let angle = 0.74_f32;
    let start = [right - stroke * 0.72, bottom - stroke * 0.72];
    let center = [
        start[0] + angle.cos() * handle_len * 0.5,
        start[1] + angle.sin() * handle_len * 0.5,
    ];
    let handle = [
        center[0] - handle_len * 0.5,
        center[1] - stroke * 0.5,
        handle_len,
        stroke,
    ];
    let mut mark = inst_radii(handle, color, [radius; 4]);
    mark.transform2[0] = angle;
    out.push(mark);
    out.push(inst_radii(
        [
            start[0] - stroke * 0.58,
            start[1] - stroke * 0.58,
            stroke * 1.4,
            stroke * 1.4,
        ],
        color,
        [stroke * 0.7; 4],
    ));
}

fn emit_line_plot_fit_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.35 * sf).max(1.0);
    let len = (5.6 * sf).min(w.min(h) * 0.32).max(stroke * 2.2);
    let inset = (6.0 * sf).min(w.min(h) * 0.28);
    let radius = stroke * 0.5;
    for (cx, cy, sx, sy) in [
        (x + inset, y + inset, 1.0, 1.0),
        (x + w - inset, y + inset, -1.0, 1.0),
        (x + w - inset, y + h - inset, -1.0, -1.0),
        (x + inset, y + h - inset, 1.0, -1.0),
    ] {
        out.push(inst_radii(
            [
                if sx > 0.0 { cx } else { cx - len },
                cy - stroke * 0.5,
                len,
                stroke,
            ],
            color,
            [radius; 4],
        ));
        out.push(inst_radii(
            [
                cx - stroke * 0.5,
                if sy > 0.0 { cy } else { cy - len },
                stroke,
                len,
            ],
            color,
            [radius; 4],
        ));
    }
}

fn emit_line_plot_grid_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.25 * sf).max(1.0);
    let size = (12.0 * sf).min(w.min(h) * 0.58).max(stroke * 6.0);
    let left = x + (w - size) * 0.5;
    let top = y + (h - size) * 0.5;
    let radius = stroke * 0.5;
    for i in 0..=2 {
        let t = i as f32 / 2.0;
        let gx = left + size * t;
        let gy = top + size * t;
        out.push(inst_radii(
            [gx - stroke * 0.5, top, stroke, size],
            color,
            [radius; 4],
        ));
        out.push(inst_radii(
            [left, gy - stroke * 0.5, size, stroke],
            color,
            [radius; 4],
        ));
    }
}

fn emit_line_plot_axes_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.45 * sf).max(1.0);
    let size = (12.0 * sf).min(w.min(h) * 0.58).max(stroke * 5.0);
    let left = x + (w - size) * 0.5;
    let top = y + (h - size) * 0.5;
    let bottom = top + size;
    let radius = stroke * 0.5;
    out.push(inst_radii(
        [left, bottom - stroke, size, stroke],
        color,
        [radius; 4],
    ));
    out.push(inst_radii([left, top, stroke, size], color, [radius; 4]));
    let tick = (3.2 * sf).max(stroke * 1.6);
    for t in [0.38, 0.68] {
        let tx = left + size * t;
        let ty = bottom - size * t;
        out.push(inst_radii(
            [tx - stroke * 0.5, bottom - stroke, stroke, tick],
            color,
            [radius; 4],
        ));
        out.push(inst_radii(
            [left - tick + stroke, ty - stroke * 0.5, tick, stroke],
            color,
            [radius; 4],
        ));
    }
}

pub(crate) fn line_plot_text_labels(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) -> Vec<LinePlotTextLabel> {
    let mut labels = Vec::new();
    let tick_color = mix(theme.muted_text, theme.text, 0.18);
    let tick_color = Some([tick_color[0], tick_color[1], tick_color[2]]);

    let plot = line_plot_plot_rect(node, sf, rect);
    if line_plot_axis_labels_enabled(node, rect) {
        let axis_color = mix(theme.muted_text, theme.text, 0.72);
        let axis_color = Some([axis_color[0], axis_color[1], axis_color[2]]);
        let pad = line_plot_outer_padding(node, sf, rect);
        if let Some(label) = node.props.line_plot_x_label.as_deref() {
            labels.push(LinePlotTextLabel {
                text: label.to_string(),
                screen_x: plot[0] + plot[2] * 0.5,
                screen_y: rect[1] + rect[3] - pad,
                is_title: true,
                anchor: "plot-x-label",
                color: axis_color,
                font_size: Some(LINE_PLOT_AXIS_LABEL_FONT_SIZE_LP),
                clip_rect: None,
            });
        }
        if let Some(label) = line_plot_y_axis_label(node) {
            labels.push(LinePlotTextLabel {
                text: label.to_string(),
                screen_x: rect[0] + pad + LINE_PLOT_AXIS_LABEL_GUTTER_LP * sf * 0.5,
                screen_y: plot[1] + plot[3] * 0.5,
                is_title: true,
                anchor: "plot-y-label",
                color: axis_color,
                font_size: Some(LINE_PLOT_AXIS_LABEL_FONT_SIZE_LP),
                clip_rect: None,
            });
        }
    }

    let Some(bounds) = line_plot_resolved_bounds(node) else {
        return labels;
    };
    let tick_count = node.props.line_plot_tick_count.clamp(2, 9);
    let x_ticks = line_plot_ticks(bounds.x_min, bounds.x_max, tick_count);
    let y_ticks = line_plot_ticks(bounds.y_min, bounds.y_max, tick_count);
    let x_step = x_ticks
        .windows(2)
        .next()
        .map(|pair| (pair[1] - pair[0]).abs())
        .unwrap_or_else(|| (bounds.x_max - bounds.x_min).abs());
    let y_step = y_ticks
        .windows(2)
        .next()
        .map(|pair| (pair[1] - pair[0]).abs())
        .unwrap_or_else(|| (bounds.y_max - bounds.y_min).abs());

    if line_plot_ticks_enabled(node, rect) {
        for tick in x_ticks {
            let t = ((tick - bounds.x_min) / (bounds.x_max - bounds.x_min).max(f32::EPSILON))
                .clamp(0.0, 1.0);
            labels.push(LinePlotTextLabel {
                text: format_line_plot_tick(tick, x_step),
                screen_x: plot[0] + plot[2] * t,
                screen_y: plot[1] + plot[3] + 7.0 * sf,
                is_title: false,
                anchor: "plot-x-tick",
                color: tick_color,
                font_size: Some(10.0),
                clip_rect: None,
            });
        }
        for tick in y_ticks {
            let t = ((tick - bounds.y_min) / (bounds.y_max - bounds.y_min).max(f32::EPSILON))
                .clamp(0.0, 1.0);
            labels.push(LinePlotTextLabel {
                text: format_line_plot_tick(tick, y_step),
                screen_x: plot[0] - 2.0 * sf,
                screen_y: plot[1] + plot[3] * (1.0 - t),
                is_title: false,
                anchor: "plot-y-tick",
                color: tick_color,
                font_size: Some(10.0),
                clip_rect: None,
            });
        }
    }

    if let Some(hover) = node.props.line_plot_hover.as_ref() {
        if hover.screen[0] < plot[0]
            || hover.screen[0] > plot[0] + plot[2]
            || hover.screen[1] < plot[1]
            || hover.screen[1] > plot[1] + plot[3]
        {
            return labels;
        }
        let text = format!(
            "{}x {}, y {}",
            hover
                .label
                .as_deref()
                .filter(|label| !label.is_empty())
                .map(|label| format!("{label}: "))
                .unwrap_or_default(),
            format_line_plot_hover_value(hover.plot[0]),
            format_line_plot_hover_value(hover.plot[1])
        );
        let clip_rect = line_plot_hover_readout_rect(hover.screen, plot, sf);
        let color = mix(theme.text, theme.accent, 0.12);
        labels.push(LinePlotTextLabel {
            text,
            screen_x: clip_rect[0] + clip_rect[2] * 0.5,
            screen_y: clip_rect[1] + clip_rect[3] * 0.5,
            is_title: false,
            anchor: "plot-readout",
            color: Some([color[0], color[1], color[2]]),
            font_size: Some(10.0),
            clip_rect: Some(clip_rect),
        });
    }

    if let Some(legend_rect) = line_plot_legend_rect(node, theme, plot, sf) {
        let entries = line_plot_legend_entries(node, theme);
        let color = mix(theme.text, theme.muted_text, 0.08);
        let color = Some([color[0], color[1], color[2]]);
        let row_h = 17.0 * sf;
        let content_h = entries.len() as f32 * row_h;
        let mut cy = legend_rect[1] + (legend_rect[3] - content_h).max(0.0) * 0.5 + row_h * 0.5;
        for (label, _, _) in entries {
            labels.push(LinePlotTextLabel {
                text: label,
                screen_x: legend_rect[0] + 35.0 * sf,
                screen_y: cy - 7.5 * sf,
                is_title: false,
                anchor: "top-left",
                color,
                font_size: Some(10.0),
                clip_rect: Some([
                    legend_rect[0] + 34.0 * sf,
                    cy - 9.0 * sf,
                    (legend_rect[2] - 39.0 * sf).max(12.0 * sf),
                    16.0 * sf,
                ]),
            });
            cy += row_h;
        }
    }

    labels
}

pub(crate) fn pie_chart_text_labels(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
) -> Vec<LinePlotTextLabel> {
    let mut labels = Vec::new();
    let color = mix(theme.muted_text, theme.text, 0.5);
    let color = Some([color[0], color[1], color[2]]);
    if let Some(title) = node.props.pie_chart.title.as_deref() {
        labels.push(LinePlotTextLabel {
            text: title.to_string(),
            screen_x: rect[0] + 14.0 * sf,
            screen_y: rect[1] + 10.0 * sf,
            is_title: true,
            anchor: "top-left",
            color,
            font_size: Some(13.0),
            clip_rect: Some(rect),
        });
    }
    let chart = pie_chart_chart_rect(node, sf, rect);
    let plot = pie_chart_plot_rect(chart, sf);
    let total = node.props.pie_chart.total.max(f32::EPSILON);
    let label_color = pie_chart_label_text_color(node, theme);
    let label_font_size = pie_chart_label_font_size(node).unwrap_or(10.0);
    if node.props.pie_chart.donut
        && (node.props.pie_chart.center_value.is_some()
            || node.props.pie_chart.center_label.is_some())
    {
        let size = plot[2].min(plot[3]).max(1.0);
        let inner = (size * node.props.pie_chart.inner_radius).max(34.0 * sf);
        let cx = plot[0] + plot[2] * 0.5;
        let cy = plot[1] + plot[3] * 0.5;
        if let Some(value) = node.props.pie_chart.center_value.as_deref() {
            let center_color = mix(theme.text, theme.accent, 0.06);
            let font_size = (inner * 0.18).clamp(14.0, 24.0);
            labels.push(LinePlotTextLabel {
                text: value.to_string(),
                screen_x: cx,
                screen_y: cy - 13.0 * sf,
                is_title: true,
                anchor: "box-center",
                color: Some([center_color[0], center_color[1], center_color[2]]),
                font_size: Some(font_size),
                clip_rect: Some([cx - inner * 0.48, cy - 23.0 * sf, inner * 0.96, 26.0 * sf]),
            });
        }
        if let Some(caption) = node.props.pie_chart.center_label.as_deref() {
            let caption_color = mix(theme.muted_text, theme.text, 0.32);
            labels.push(LinePlotTextLabel {
                text: caption.to_string(),
                screen_x: cx,
                screen_y: cy + 9.0 * sf,
                is_title: false,
                anchor: "box-center",
                color: Some([caption_color[0], caption_color[1], caption_color[2]]),
                font_size: Some(10.5),
                clip_rect: Some([cx - inner * 0.50, cy + 4.0 * sf, inner, 18.0 * sf]),
            });
        }
    }
    if node.props.pie_chart.show_labels && node.props.pie_chart.label_mode != "none" {
        for label in pie_chart_slice_label_layouts(node, theme, sf, plot) {
            labels.push(LinePlotTextLabel {
                text: label.text,
                screen_x: label.screen_x,
                screen_y: label.screen_y,
                is_title: false,
                anchor: "box-center",
                color: label_color.or(Some([0.98, 0.99, 1.0])),
                font_size: Some(label_font_size),
                clip_rect: Some(label.rect),
            });
        }
    }
    if node.props.pie_chart.show_legend && node.props.pie_chart.legend_position != "none" {
        let legend = pie_chart_legend_rect(node, sf, rect, chart);
        let legend_font_size = label_font_size.max(10.0);
        let line_h = legend_font_size * 1.3 * sf;
        let row_h = (line_h + 6.0 * sf).max(20.0 * sf);
        for (index, slice) in node.props.pie_chart.slices.iter().enumerate() {
            let y = legend[1] + 8.0 * sf + index as f32 * row_h;
            if y + row_h > legend[1] + legend[3] {
                break;
            }
            let row_color = label_color.or(color);
            let percent = format!("{:.0}%", slice.value / total * 100.0);
            let text_y = y + (row_h - line_h).max(0.0) * 0.5;
            labels.push(LinePlotTextLabel {
                text: slice.label.clone(),
                screen_x: legend[0] + 24.0 * sf,
                screen_y: text_y,
                is_title: false,
                anchor: "top-left",
                color: row_color,
                font_size: Some(legend_font_size),
                clip_rect: Some([
                    legend[0] + 23.0 * sf,
                    y,
                    (legend[2] - 70.0 * sf).max(12.0 * sf),
                    row_h,
                ]),
            });
            labels.push(LinePlotTextLabel {
                text: percent,
                screen_x: legend[0] + legend[2] - 43.0 * sf,
                screen_y: text_y,
                is_title: false,
                anchor: "top-left",
                color: row_color,
                font_size: Some(legend_font_size),
                clip_rect: Some([legend[0] + legend[2] - 44.0 * sf, y, 40.0 * sf, row_h]),
            });
        }
    }
    labels
}

fn emit_line_plot_series(
    out: &mut Vec<RectInstance>,
    points: &[[f32; 2]],
    plot: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    line_style: &str,
) {
    let (start, end) = line_plot_visible_point_bounds(points, bounds);
    if end.saturating_sub(start) < 2 {
        return;
    }
    let visible = &points[start..end];
    let segment_count = visible.len().saturating_sub(1).max(1);
    let stride = ((segment_count + LINE_PLOT_MAX_SEGMENTS_PER_SERIES - 1)
        / LINE_PLOT_MAX_SEGMENTS_PER_SERIES)
        .max(1);
    let mut state = LinePlotEmitState::default();
    let mut last_index = 0usize;
    for idx in (0..visible.len()).step_by(stride) {
        emit_line_plot_point(
            out,
            visible[idx],
            plot,
            bounds,
            line_width,
            color,
            line_style,
            &mut state,
        );
        last_index = idx;
    }
    if last_index != visible.len() - 1 {
        emit_line_plot_point(
            out,
            visible[visible.len() - 1],
            plot,
            bounds,
            line_width,
            color,
            line_style,
            &mut state,
        );
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LinePlotEmitState {
    before_previous: Option<[f32; 2]>,
    previous: Option<[f32; 2]>,
}

fn emit_line_plot_point(
    out: &mut Vec<RectInstance>,
    point: [f32; 2],
    plot: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    line_style: &str,
    state: &mut LinePlotEmitState,
) {
    let mapped = map_line_plot_point(point, plot, bounds);
    let Some(mapped) = mapped else {
        *state = LinePlotEmitState::default();
        return;
    };
    if let Some(previous) = state.previous {
        if let Some((start, end)) = clip_line_segment_to_rect(previous, mapped, plot) {
            push_styled_line_segment(out, start, end, line_width, color, line_style);
            if matches!(line_style, "solid" | "") {
                push_line_join_if_needed(
                    out,
                    state.before_previous,
                    previous,
                    mapped,
                    plot,
                    line_width,
                    color,
                );
            }
        }
    }
    state.before_previous = state.previous;
    state.previous = Some(mapped);
}

fn map_line_plot_point(
    point: [f32; 2],
    plot: [f32; 4],
    bounds: LinePlotBounds,
) -> Option<[f32; 2]> {
    let [px, py] = point;
    if !px.is_finite() || !py.is_finite() {
        return None;
    }
    let x_range = (bounds.x_max - bounds.x_min).max(f32::EPSILON);
    let y_range = (bounds.y_max - bounds.y_min).max(f32::EPSILON);
    let tx = (px - bounds.x_min) / x_range;
    let ty = (py - bounds.y_min) / y_range;
    Some([plot[0] + plot[2] * tx, plot[1] + plot[3] * (1.0 - ty)])
}

fn clip_line_segment_to_rect(
    start: [f32; 2],
    end: [f32; 2],
    rect: [f32; 4],
) -> Option<([f32; 2], [f32; 2])> {
    let [left, top, width, height] = rect;
    let right = left + width;
    let bottom = top + height;
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let mut t0 = 0.0_f32;
    let mut t1 = 1.0_f32;

    fn test_edge(p: f32, q: f32, t0: &mut f32, t1: &mut f32) -> bool {
        if p.abs() <= f32::EPSILON {
            return q >= 0.0;
        }
        let r = q / p;
        if p < 0.0 {
            if r > *t1 {
                return false;
            }
            *t0 = (*t0).max(r);
        } else {
            if r < *t0 {
                return false;
            }
            *t1 = (*t1).min(r);
        }
        true
    }

    if !test_edge(-dx, start[0] - left, &mut t0, &mut t1)
        || !test_edge(dx, right - start[0], &mut t0, &mut t1)
        || !test_edge(-dy, start[1] - top, &mut t0, &mut t1)
        || !test_edge(dy, bottom - start[1], &mut t0, &mut t1)
        || t0 > t1
    {
        return None;
    }

    Some((
        [start[0] + dx * t0, start[1] + dy * t0],
        [start[0] + dx * t1, start[1] + dy * t1],
    ))
}

fn push_line_segment(
    out: &mut Vec<RectInstance>,
    start: [f32; 2],
    end: [f32; 2],
    width: f32,
    color: [f32; 4],
) {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.001 {
        let radius = width * 0.5;
        let mut dot = inst_radii(
            [start[0] - radius, start[1] - radius, width, width],
            color,
            [radius; 4],
        );
        dot.transform2[3] = 1.0;
        out.push(dot);
        return;
    }
    let cx = (start[0] + end[0]) * 0.5;
    let cy = (start[1] + end[1]) * 0.5;
    let mut segment = inst_radii(
        [cx - len * 0.5, cy - width * 0.5, len, width],
        color,
        [width * 0.5; 4],
    );
    segment.transform2[0] = dy.atan2(dx);
    segment.transform2[3] = 1.0;
    out.push(segment);
}

fn push_line_join_if_needed(
    out: &mut Vec<RectInstance>,
    before: Option<[f32; 2]>,
    joint: [f32; 2],
    after: [f32; 2],
    plot: [f32; 4],
    width: f32,
    color: [f32; 4],
) {
    let Some(before) = before else {
        return;
    };
    if joint[0] < plot[0]
        || joint[0] > plot[0] + plot[2]
        || joint[1] < plot[1]
        || joint[1] > plot[1] + plot[3]
    {
        return;
    }
    let v0 = [joint[0] - before[0], joint[1] - before[1]];
    let v1 = [after[0] - joint[0], after[1] - joint[1]];
    let len0 = (v0[0] * v0[0] + v0[1] * v0[1]).sqrt();
    let len1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt();
    if len0 <= 0.25 || len1 <= 0.25 {
        return;
    }
    let dot = ((v0[0] * v1[0] + v0[1] * v1[1]) / (len0 * len1)).clamp(-1.0, 1.0);
    if dot >= LINE_PLOT_JOIN_DOT_COS_THRESHOLD {
        return;
    }
    let radius = width * 0.5;
    let mut join = inst_radii(
        [joint[0] - radius, joint[1] - radius, width, width],
        color,
        [radius; 4],
    );
    join.transform2[3] = 1.0;
    out.push(join);
}

fn push_styled_line_segment(
    out: &mut Vec<RectInstance>,
    start: [f32; 2],
    end: [f32; 2],
    width: f32,
    color: [f32; 4],
    line_style: &str,
) {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.25 || matches!(line_style, "solid" | "") {
        push_line_segment(out, start, end, width, color);
        return;
    }
    let dir = [dx / len, dy / len];
    let pattern: &[(f32, bool)] = match line_style {
        "dotted" => &[(1.2, true), (4.0, false)],
        "dashdot" => &[(8.0, true), (4.0, false), (1.4, true), (4.0, false)],
        "dashed" => &[(9.0, true), (5.0, false)],
        _ => &[(len, true)],
    };
    let mut cursor = 0.0_f32;
    let mut index = 0usize;
    let min_on = width.max(1.0);
    while cursor < len {
        let (units, draw) = pattern[index % pattern.len()];
        let seg_len = (units * width.max(1.0)).max(min_on);
        let next = (cursor + seg_len).min(len);
        if draw && next > cursor {
            let a = [start[0] + dir[0] * cursor, start[1] + dir[1] * cursor];
            let b = [start[0] + dir[0] * next, start[1] + dir[1] * next];
            push_line_segment(out, a, b, width, color);
        }
        cursor = next;
        index += 1;
        if index > 2048 {
            break;
        }
    }
}

fn part_style_mark_color(style: &PartStyle, theme: &Theme) -> Option<Color> {
    let color = style
        .text
        .color
        .as_ref()
        .or(style.visual.foreground.as_ref())?;
    let mut resolved = color.resolve(theme);
    if let Some(opacity) = style.visual.opacity {
        resolved[3] *= opacity.clamp(0.0, 1.0);
    }
    Some(resolved)
}

fn number_stepper_mark_color(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    part: &str,
) -> Color {
    let fallback = if state.is_disabled(&node.id) {
        theme.disabled
    } else {
        theme.muted_text
    };
    let parts = [part, "stepper"];
    for part in parts {
        if let Some(color) = state_part_style_for_state(&node.style, &node.id, state, part)
            .and_then(|style| part_style_mark_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = checked_part_style_for_state(&node.style, &node.id, state, part)
            .and_then(|style| part_style_mark_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = open_part_style_for_state(&node.style, &node.id, state, part)
            .and_then(|style| part_style_mark_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = expanded_part_style_for_state(&node.style, &node.id, state, part)
            .and_then(|style| part_style_mark_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = collapsed_part_style_for_state(&node.style, &node.id, state, part)
            .and_then(|style| part_style_mark_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = selected_part_style_for_state(&node.style, &node.id, state, part)
            .and_then(|style| part_style_mark_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) =
            base_part_style(&node.style, part).and_then(|style| part_style_mark_color(style, theme))
        {
            return color;
        }
    }
    fallback
}

fn single_part_mark_color(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    part: &str,
    fallback: Color,
) -> Color {
    let fallback = if state.is_disabled(&node.id) {
        theme.disabled
    } else {
        fallback
    };
    state_part_style_for_state(&node.style, &node.id, state, part)
        .and_then(|style| part_style_mark_color(style, theme))
        .or_else(|| {
            checked_part_style_for_state(&node.style, &node.id, state, part)
                .and_then(|style| part_style_mark_color(style, theme))
        })
        .or_else(|| {
            open_part_style_for_state(&node.style, &node.id, state, part)
                .and_then(|style| part_style_mark_color(style, theme))
        })
        .or_else(|| {
            expanded_part_style_for_state(&node.style, &node.id, state, part)
                .and_then(|style| part_style_mark_color(style, theme))
        })
        .or_else(|| {
            collapsed_part_style_for_state(&node.style, &node.id, state, part)
                .and_then(|style| part_style_mark_color(style, theme))
        })
        .or_else(|| {
            selected_part_style_for_state(&node.style, &node.id, state, part)
                .and_then(|style| part_style_mark_color(style, theme))
        })
        .or_else(|| {
            base_part_style(&node.style, part).and_then(|style| part_style_mark_color(style, theme))
        })
        .unwrap_or(fallback)
}

fn emit_stepper_mark(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    plus: bool,
    sf: f32,
) {
    let [x, y, w, h] = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let stroke = (1.5 * sf).max(1.0).min(h * 0.18);
    let mark = w.min(h).mul_add(0.34, 0.0).max(stroke * 3.0);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let radius = stroke * 0.5;
    out.push(inst_radii(
        [cx - mark * 0.5, cy - stroke * 0.5, mark, stroke],
        color,
        [radius; 4],
    ));
    if plus {
        out.push(inst_radii(
            [cx - stroke * 0.5, cy - mark * 0.5, stroke, mark],
            color,
            [radius; 4],
        ));
    }
}

fn widget_raw_str<'a>(node: &'a WidgetNode, key: &str) -> Option<&'a str> {
    node.props
        .raw_props
        .get(key)
        .and_then(|value| value.as_str())
}

fn nav_item_uses_compact_icon(node: &WidgetNode, width: f32, sf: f32) -> bool {
    node.kind == WidgetKind::NavItem && widget_raw_str(node, "icon").is_some() && width <= 72.0 * sf
}

fn emit_tool_icon_button_mark(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    rect: [f32; 4],
    color: Color,
    sf: f32,
    icon_geometry_cache: &mut IconGeometryCache,
) {
    if let Some(resource) = node.props.raw_props.get("icon_override_resource") {
        if let Some(geometry) = icon_geometry_cache.resolve(resource) {
            emit_custom_icon_geometry(out, &geometry, rect, color);
            return;
        }
    }
    let requested = widget_raw_str(node, "icon_override_name")
        .or_else(|| widget_raw_str(node, "icon"))
        .unwrap_or("more");
    let icon = crate::icons::resolve_builtin_icon(requested).resolved;
    match icon {
        "add" => emit_stepper_mark(out, rect, color, true, sf),
        "minus" => emit_stepper_mark(out, rect, color, false, sf),
        "close" => emit_tool_x_icon(out, rect, color, sf),
        "check" => emit_tool_check_icon(out, rect, color, sf),
        "edit" => emit_tool_edit_icon(out, rect, color, sf),
        "copy"
            if node
                .css_types
                .iter()
                .any(|css_type| css_type == "WindowMaximize") =>
        {
            emit_window_restore_icon(out, rect, color, sf)
        }
        "copy" => emit_tool_copy_icon(out, rect, color, sf),
        "file" => emit_tool_file_icon(out, rect, color, sf),
        "folder" => emit_tool_folder_icon(out, rect, color, sf),
        "upload" => emit_tool_transfer_icon(out, rect, color, true, sf),
        "download" => emit_tool_transfer_icon(out, rect, color, false, sf),
        "refresh" => emit_tool_refresh_icon(out, rect, color, sf),
        "settings" => emit_tool_settings_icon(out, rect, color, sf),
        "home" => emit_tool_home_icon(out, rect, color, sf),
        "info" => emit_tool_info_icon(out, rect, color, sf),
        "help" => emit_tool_help_icon(out, rect, color, sf),
        "warning" => emit_tool_warning_icon(out, rect, color, sf),
        "lock" => emit_tool_lock_icon(out, rect, color, false, sf),
        "unlock" => emit_tool_lock_icon(out, rect, color, true, sf),
        "eye" => emit_tool_eye_icon(out, rect, color, false, sf),
        "eye-off" => emit_tool_eye_icon(out, rect, color, true, sf),
        "menu" => emit_tool_menu_icon(out, rect, color, sf),
        "list" => emit_tool_list_icon(out, rect, color, sf),
        "filter" => emit_tool_filter_icon(out, rect, color, sf),
        "sort" => emit_tool_sort_icon(out, rect, color, sf),
        "undo" => emit_tool_history_icon(out, rect, color, true, sf),
        "redo" => emit_tool_history_icon(out, rect, color, false, sf),
        "play" => emit_tool_triangle_icon(out, rect, color, "right", sf),
        "pause" => emit_tool_pause_icon(out, rect, color, sf),
        "stop" => emit_tool_stop_icon(out, rect, color, sf),
        "save" => emit_tool_save_icon(out, rect, color, sf),
        "search" => emit_line_plot_zoom_icon(out, rect, color, sf),
        "fit" => emit_line_plot_fit_icon(out, rect, color, sf),
        "pan" | "move" => emit_line_plot_pan_icon(out, rect, color, sf),
        "grid" => emit_line_plot_grid_icon(out, rect, color, sf),
        "axes" => emit_line_plot_axes_icon(out, rect, color, sf),
        "more" | _ => emit_tool_more_icon(out, rect, color, sf),
    }
}

fn parse_custom_icon_resource(resource: &Value) -> Option<ParsedIconGeometry> {
    let Some(view_box) = resource.get("view_box").and_then(Value::as_array) else {
        return None;
    };
    let Some(strokes) = resource.get("strokes").and_then(Value::as_array) else {
        return None;
    };
    if view_box.len() != 4 || strokes.is_empty() {
        return None;
    }
    let values = view_box
        .iter()
        .map(|value| value.as_f64().map(|value| value as f32))
        .collect::<Option<Vec<_>>>();
    let Some(values) = values else {
        return None;
    };
    let [view_x, view_y, view_w, view_h] = [values[0], values[1], values[2], values[3]];
    if view_w <= 0.0 || view_h <= 0.0 {
        return None;
    }
    let Some(resource_stroke_width) = resource
        .get("stroke_width")
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| *value > 0.0)
    else {
        return None;
    };
    let mut parsed_strokes = Vec::with_capacity(strokes.len());
    for stroke in strokes {
        let points = stroke.get("points").and_then(Value::as_array)?;
        let parsed_points = points
            .iter()
            .map(|point| {
                let pair = point.as_array()?;
                if pair.len() != 2 {
                    return None;
                }
                Some([pair[0].as_f64()? as f32, pair[1].as_f64()? as f32])
            })
            .collect::<Option<Vec<_>>>()?;
        if parsed_points.len() < 2 {
            return None;
        }
        parsed_strokes.push(ParsedIconStroke {
            points: parsed_points,
            closed: stroke.get("closed").and_then(Value::as_bool) == Some(true),
        });
    }
    Some(ParsedIconGeometry {
        view_box: [view_x, view_y, view_w, view_h],
        stroke_width: resource_stroke_width,
        strokes: parsed_strokes,
    })
}

fn emit_custom_icon_geometry(
    out: &mut Vec<RectInstance>,
    geometry: &ParsedIconGeometry,
    rect: [f32; 4],
    color: Color,
) {
    let [view_x, view_y, view_w, view_h] = geometry.view_box;
    let target_side = rect[2].min(rect[3]) * 0.56;
    let scale = (target_side / view_w).min(target_side / view_h);
    let target_w = view_w * scale;
    let target_h = view_h * scale;
    let origin_x = rect[0] + (rect[2] - target_w) * 0.5 - view_x * scale;
    let origin_y = rect[1] + (rect[3] - target_h) * 0.5 - view_y * scale;
    let stroke_width = (geometry.stroke_width * scale).max(1.0);
    let transform = |point: [f32; 2]| [origin_x + point[0] * scale, origin_y + point[1] * scale];
    for stroke in &geometry.strokes {
        for segment in stroke.points.windows(2) {
            push_line_segment(
                out,
                transform(segment[0]),
                transform(segment[1]),
                stroke_width,
                color,
            );
        }
        if stroke.closed && stroke.points.len() >= 3 {
            push_line_segment(
                out,
                transform(*stroke.points.last().expect("non-empty icon stroke")),
                transform(stroke.points[0]),
                stroke_width,
                color,
            );
        }
    }
}

fn emit_arrow_button_mark(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    rect: [f32; 4],
    color: Color,
    sf: f32,
) {
    let direction = widget_raw_str(node, "direction").unwrap_or("right");
    emit_tool_triangle_icon(out, rect, color, direction, sf);
}

fn emit_tool_triangle_icon(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    direction: &str,
    sf: f32,
) {
    let [x, y, w, h] = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let mark = (w.min(h) * 0.42).max(8.0 * sf);
    let mut tri = inst_rounded_triangle(
        [x + (w - mark) * 0.5, y + (h - mark) * 0.5, mark, mark],
        color,
        !matches!(direction, "down"),
        (1.0 * sf).max(0.75),
    );
    tri.transform2[0] = match direction {
        "left" => -std::f32::consts::FRAC_PI_2,
        "right" => std::f32::consts::FRAC_PI_2,
        _ => 0.0,
    };
    out.push(tri);
}

fn emit_tool_arrow_head(
    out: &mut Vec<RectInstance>,
    center: [f32; 2],
    size: f32,
    direction: &str,
    color: Color,
    sf: f32,
) {
    let mut tri = inst_rounded_triangle(
        [center[0] - size * 0.5, center[1] - size * 0.5, size, size],
        color,
        !matches!(direction, "down"),
        (0.8 * sf).max(0.5),
    );
    tri.transform2[0] = match direction {
        "left" => -std::f32::consts::FRAC_PI_2,
        "right" => std::f32::consts::FRAC_PI_2,
        _ => 0.0,
    };
    out.push(tri);
}

fn emit_tool_arrow_head_angle(
    out: &mut Vec<RectInstance>,
    center: [f32; 2],
    size: f32,
    angle: f32,
    color: Color,
    sf: f32,
) {
    let mut tri = inst_rounded_triangle(
        [center[0] - size * 0.5, center[1] - size * 0.5, size, size],
        color,
        true,
        (0.8 * sf).max(0.5),
    );
    tri.transform2[0] = angle + std::f32::consts::FRAC_PI_2;
    out.push(tri);
}

fn emit_tool_bar(
    out: &mut Vec<RectInstance>,
    center: [f32; 2],
    len: f32,
    stroke: f32,
    angle: f32,
    color: Color,
) {
    let radius = stroke * 0.5;
    let mut mark = inst_radii(
        [center[0] - len * 0.5, center[1] - stroke * 0.5, len, stroke],
        color,
        [radius; 4],
    );
    mark.transform2[0] = angle;
    out.push(mark);
}

fn emit_tool_line_rect(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, stroke: f32) {
    let [x, y, w, h] = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let radius = stroke * 0.5;
    out.push(inst_radii([x, y, w, stroke], color, [radius; 4]));
    out.push(inst_radii(
        [x, y + h - stroke, w, stroke],
        color,
        [radius; 4],
    ));
    out.push(inst_radii([x, y, stroke, h], color, [radius; 4]));
    out.push(inst_radii(
        [x + w - stroke, y, stroke, h],
        color,
        [radius; 4],
    ));
}

fn emit_tool_x_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let len = (w.min(h) * 0.42).max(8.0 * sf);
    let stroke = (1.55 * sf).max(1.1);
    let center = [x + w * 0.5, y + h * 0.5];
    emit_tool_bar(out, center, len, stroke, std::f32::consts::FRAC_PI_4, color);
    emit_tool_bar(
        out,
        center,
        len,
        stroke,
        -std::f32::consts::FRAC_PI_4,
        color,
    );
}

fn emit_tool_check_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.65 * sf).max(1.1);
    let cx = x + w * 0.5;
    let cy = y + h * 0.54;
    emit_tool_bar(
        out,
        [cx - 2.6 * sf, cy + 1.4 * sf],
        5.0 * sf,
        stroke,
        std::f32::consts::FRAC_PI_4,
        color,
    );
    emit_tool_bar(
        out,
        [cx + 2.4 * sf, cy - 1.8 * sf],
        9.0 * sf,
        stroke,
        -std::f32::consts::FRAC_PI_4,
        color,
    );
}

fn emit_tool_edit_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (2.0 * sf).max(1.2);
    let len = (w.min(h) * 0.58).max(11.0 * sf);
    let cx = x + w * 0.50;
    let cy = y + h * 0.50;
    emit_tool_bar(
        out,
        [cx, cy],
        len,
        stroke,
        -std::f32::consts::FRAC_PI_4,
        color,
    );
    emit_tool_bar(
        out,
        [cx - len * 0.32, cy + len * 0.32],
        (5.0 * sf).max(stroke * 2.0),
        stroke,
        0.0,
        color,
    );
    let cap = (3.6 * sf).max(stroke * 1.4);
    out.push(inst_radii(
        [
            cx + len * 0.28 - cap * 0.5,
            cy - len * 0.28 - cap * 0.5,
            cap,
            cap,
        ],
        color,
        [cap * 0.18; 4],
    ));
}

fn emit_tool_file_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.35 * sf).max(1.0);
    let doc_w = (w.min(h) * 0.48).max(10.0 * sf);
    let doc_h = (doc_w * 1.22).min(h * 0.66).max(12.0 * sf);
    let left = x + (w - doc_w) * 0.5;
    let top = y + (h - doc_h) * 0.5;
    emit_tool_line_rect(out, [left, top, doc_w, doc_h], color, stroke);
    let fold = (doc_w * 0.26).max(3.0 * sf);
    emit_tool_bar(
        out,
        [left + doc_w - fold * 0.52, top + fold * 0.52],
        fold * 1.32,
        stroke,
        std::f32::consts::FRAC_PI_4,
        color,
    );
    for offset in [0.44, 0.62] {
        out.push(inst_radii(
            [
                left + doc_w * 0.24,
                top + doc_h * offset,
                doc_w * 0.52,
                stroke,
            ],
            color,
            [stroke * 0.5; 4],
        ));
    }
}

fn emit_tool_copy_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.25 * sf).max(1.0);
    let doc_w = (w.min(h) * 0.42).max(9.0 * sf);
    let doc_h = (doc_w * 1.18).max(10.0 * sf);
    let shift = (3.2 * sf).max(stroke * 2.0);
    let left = x + (w - doc_w) * 0.5 - shift * 0.35;
    let top = y + (h - doc_h) * 0.5 + shift * 0.35;
    emit_tool_line_rect(
        out,
        [left + shift, top - shift, doc_w, doc_h],
        color,
        stroke,
    );
    emit_tool_line_rect(out, [left, top, doc_w, doc_h], color, stroke);
}

fn emit_window_restore_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.15 * sf).max(1.0);
    let window_w = (w.min(h) * 0.34).max(8.0 * sf);
    let window_h = (window_w * 0.78).max(6.0 * sf);
    let shift = (2.8 * sf).max(stroke * 1.8);
    let left = x + (w - window_w - shift) * 0.5;
    let top = y + (h - window_h - shift) * 0.5;
    let radius = (1.5 * sf).max(stroke);
    for window_rect in [
        [left + shift, top, window_w, window_h],
        [left, top + shift, window_w, window_h],
    ] {
        out.push(inst_outline_ring_clipped(
            window_rect,
            color,
            [radius; 4],
            stroke,
            default_local_clip(window_rect),
        ));
    }
}

fn emit_tool_folder_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let width = (w.min(h) * 0.62).max(12.0 * sf);
    let height = (width * 0.58).max(7.0 * sf);
    let left = x + (w - width) * 0.5;
    let top = y + h * 0.5 - height * 0.42;
    let tab_w = width * 0.36;
    let tab_h = height * 0.30;
    let radius = (1.1 * sf).max(0.7);
    out.push(inst_radii(
        [left, top + tab_h, width, height],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [left + width * 0.08, top, tab_w, tab_h + (1.0 * sf).max(0.8)],
        color,
        [radius; 4],
    ));
}

fn emit_tool_transfer_icon(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    upload: bool,
    sf: f32,
) {
    let [x, y, w, h] = rect;
    let stroke = (1.45 * sf).max(1.0);
    let cx = x + w * 0.5;
    let cy = y + h * 0.50;
    let shaft = (7.2 * sf).min(h * 0.34).max(stroke * 3.0);
    let head = (5.5 * sf).max(stroke * 2.2);
    let base = (12.0 * sf).min(w * 0.56).max(stroke * 5.0);
    let radius = stroke * 0.5;
    out.push(inst_radii(
        [cx - stroke * 0.5, cy - shaft * 0.5, stroke, shaft],
        color,
        [radius; 4],
    ));
    let mut tri = inst_rounded_triangle(
        [
            cx - head * 0.5,
            if upload {
                cy - shaft * 0.5 - head * 0.62
            } else {
                cy + shaft * 0.5 - head * 0.38
            },
            head,
            head,
        ],
        color,
        true,
        (0.8 * sf).max(0.55),
    );
    if !upload {
        tri.transform2[0] = std::f32::consts::PI;
    }
    out.push(tri);
    out.push(inst_radii(
        [cx - base * 0.5, y + h * 0.70, base, stroke],
        color,
        [radius; 4],
    ));
}

fn emit_tool_refresh_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.55 * sf).max(1.1);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let radius = (w.min(h) * 0.265).max(stroke * 4.2);
    let inner_ratio = ((radius - stroke) / radius).clamp(0.58, 0.86);
    let head = (w.min(h) * 0.16).max(stroke * 2.3);
    for (start, sweep) in [
        (std::f32::consts::PI * 1.03, 2.72_f32),
        (0.02_f32, 2.72_f32),
    ] {
        out.push(inst_loading_spinner(
            [cx - radius, cy - radius, radius * 2.0, radius * 2.0],
            [0.0, 0.0, 0.0, 0.0],
            color,
            start,
            sweep,
            inner_ratio,
            1.0,
        ));
        let end = start + sweep;
        let head_center = [cx + end.cos() * radius, cy + end.sin() * radius];
        let tangent_angle = (end.cos()).atan2(-end.sin());
        emit_tool_arrow_head_angle(out, head_center, head, tangent_angle, color, sf);
    }
}

fn emit_tool_settings_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.55 * sf).max(1.1);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let radius = (w.min(h) * 0.18).max(stroke * 3.2);
    let tooth_len = (3.4 * sf).max(stroke * 1.75);
    let tooth_w = (2.15 * sf).max(stroke * 1.15);
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::FRAC_PI_4;
        emit_tool_bar(
            out,
            [
                cx + angle.cos() * (radius + tooth_len * 0.42),
                cy + angle.sin() * (radius + tooth_len * 0.42),
            ],
            tooth_len,
            tooth_w,
            angle,
            color,
        );
    }
    out.push(inst_loading_spinner(
        [cx - radius, cy - radius, radius * 2.0, radius * 2.0],
        [0.0, 0.0, 0.0, 0.0],
        color,
        0.0,
        std::f32::consts::TAU,
        0.54,
        1.0,
    ));
}

fn emit_tool_home_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let icon = w.min(h);
    let cx = x + w * 0.5;
    let body_w = (icon * 0.42).max(9.5 * sf);
    let body_h = (icon * 0.29).max(6.5 * sf);
    let body_top = y + h * 0.52;
    let body_left = cx - body_w * 0.5;
    let radius = (1.2 * sf).max(0.7);
    let chimney_w = (2.3 * sf).max(1.6);
    let chimney_h = icon * 0.17;
    out.push(inst_radii(
        [
            cx + body_w * 0.17,
            body_top - chimney_h * 0.88,
            chimney_w,
            chimney_h,
        ],
        color,
        [radius * 0.65; 4],
    ));
    let roof_w = (icon * 0.60).max(body_w * 1.24);
    let roof_h = icon * 0.42;
    out.push(inst_rounded_triangle(
        [cx - roof_w * 0.5, body_top - roof_h * 0.77, roof_w, roof_h],
        color,
        true,
        (1.0 * sf).max(0.65),
    ));
    out.push(inst_radii(
        [body_left, body_top, body_w, body_h],
        color,
        [radius; 4],
    ));
}

fn emit_tool_info_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (2.0 * sf).max(1.2);
    let cx = x + w * 0.5;
    let dot = (2.6 * sf).max(stroke * 1.1);
    out.push(inst_radii(
        [cx - dot * 0.5, y + h * 0.31 - dot * 0.5, dot, dot],
        color,
        [dot * 0.5; 4],
    ));
    out.push(inst_radii(
        [cx - stroke * 0.5, y + h * 0.43, stroke, h * 0.27],
        color,
        [stroke * 0.5; 4],
    ));
    out.push(inst_radii(
        [cx - stroke * 1.4, y + h * 0.70, stroke * 2.8, stroke],
        color,
        [stroke * 0.5; 4],
    ));
}

fn emit_tool_help_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let icon = w.min(h);
    let stroke = (1.35 * sf).max(1.0).min(icon * 0.09);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let radius = (icon * 0.285).max(stroke * 4.2);
    let inner_ratio = ((radius - stroke) / radius).clamp(0.66, 0.88);
    out.push(inst_loading_spinner(
        [cx - radius, cy - radius, radius * 2.0, radius * 2.0],
        [0.0, 0.0, 0.0, 0.0],
        color,
        0.0,
        std::f32::consts::TAU,
        inner_ratio,
        1.0,
    ));
}

fn emit_tool_warning_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let icon = w.min(h);
    let stroke = (1.45 * sf).max(1.0).min(icon * 0.095);
    let size = (icon * 0.58).max(12.0 * sf);
    let cx = x + w * 0.5;
    let top = y + (h - size) * 0.5 + icon * 0.03;
    let apex = [cx, top + size * 0.12];
    let left = [cx - size * 0.40, top + size * 0.78];
    let right = [cx + size * 0.40, top + size * 0.78];
    let side_len = ((apex[0] - left[0]).powi(2) + (apex[1] - left[1]).powi(2)).sqrt();
    let side_angle = (apex[1] - left[1]).atan2(apex[0] - left[0]);
    emit_tool_bar(
        out,
        [(apex[0] + left[0]) * 0.5, (apex[1] + left[1]) * 0.5],
        side_len,
        stroke,
        side_angle,
        color,
    );
    emit_tool_bar(
        out,
        [(apex[0] + right[0]) * 0.5, (apex[1] + right[1]) * 0.5],
        side_len,
        stroke,
        -side_angle,
        color,
    );
    emit_tool_bar(out, [cx, left[1]], right[0] - left[0], stroke, 0.0, color);
}

fn emit_tool_lock_icon(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    unlocked: bool,
    sf: f32,
) {
    let [x, y, w, h] = rect;
    let icon = w.min(h);
    let stroke = (1.65 * sf).max(1.15).min(icon * 0.10);
    let body_w = (icon * 0.44).max(9.5 * sf);
    let body_h = (icon * 0.32).max(7.0 * sf);
    let shackle_w = body_w * if unlocked { 0.72 } else { 0.56 };
    let shackle_h = icon * if unlocked { 0.28 } else { 0.25 };
    let shackle_radius_y = shackle_h * if unlocked { 0.66 } else { 0.76 };
    let total_top_extra = shackle_radius_y + stroke * 0.55;
    let total_h = total_top_extra + body_h;
    let top = y + (h - total_h) * 0.5 + total_top_extra;
    let left = x + (w - body_w) * 0.5 - if unlocked { body_w * 0.08 } else { 0.0 };
    let body_radius = (2.2 * sf).max(1.2);
    out.push(inst_radii(
        [left, top, body_w, body_h],
        color,
        [body_radius; 4],
    ));

    let shackle_cx = if unlocked {
        x + w * 0.5 + body_w * 0.28
    } else {
        x + w * 0.5
    };
    let arc_center = [
        shackle_cx,
        top - shackle_h * if unlocked { 0.16 } else { 0.02 },
    ];
    let radius = [shackle_w * 0.5, shackle_radius_y];
    emit_tool_arc(
        out,
        arc_center,
        radius,
        std::f32::consts::PI,
        std::f32::consts::TAU,
        9,
        stroke,
        color,
    );

    let left_leg_x = shackle_cx - shackle_w * 0.5 - stroke * 0.5;
    let right_leg_x = shackle_cx + shackle_w * 0.5 - stroke * 0.5;
    if unlocked {
        let leg_top = arc_center[1] - stroke * 0.20;
        let left_leg_h = (top + stroke * 0.25 - leg_top).max(stroke * 1.65);
        let right_leg_h = shackle_h * 0.28;
        out.push(inst_radii(
            [left_leg_x, leg_top, stroke, left_leg_h],
            color,
            [stroke * 0.5; 4],
        ));
        out.push(inst_radii(
            [right_leg_x, leg_top, stroke, right_leg_h],
            color,
            [stroke * 0.5; 4],
        ));
    } else {
        let leg_h = shackle_h * 0.58;
        let leg_top = (arc_center[1] - stroke * 0.1).min(top - stroke * 0.15);
        out.push(inst_radii(
            [left_leg_x, leg_top, stroke, leg_h],
            color,
            [stroke * 0.5; 4],
        ));
        out.push(inst_radii(
            [right_leg_x, leg_top, stroke, leg_h],
            color,
            [stroke * 0.5; 4],
        ));
    }

    let key_dot = (1.8 * sf).max(stroke * 0.72);
    let cutout = [0.0, 0.0, 0.0, color[3] * 0.45];
    out.push(inst_radii(
        [
            left + body_w * 0.5 - key_dot * 0.5,
            top + body_h * 0.40 - key_dot * 0.5,
            key_dot,
            key_dot,
        ],
        cutout,
        [key_dot * 0.5; 4],
    ));
    out.push(inst_radii(
        [
            left + body_w * 0.5 - stroke * 0.5,
            top + body_h * 0.48,
            stroke,
            body_h * 0.18,
        ],
        cutout,
        [stroke * 0.5; 4],
    ));
}

fn emit_tool_arc(
    out: &mut Vec<RectInstance>,
    center: [f32; 2],
    radius: [f32; 2],
    start: f32,
    end: f32,
    segments: usize,
    stroke: f32,
    color: Color,
) {
    if segments == 0 {
        return;
    }
    let mut prev = [
        center[0] + start.cos() * radius[0],
        center[1] + start.sin() * radius[1],
    ];
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let angle = start + (end - start) * t;
        let next = [
            center[0] + angle.cos() * radius[0],
            center[1] + angle.sin() * radius[1],
        ];
        let dx = next[0] - prev[0];
        let dy = next[1] - prev[1];
        let len = (dx * dx + dy * dy).sqrt() + stroke * 0.24;
        if len > 0.0 {
            emit_tool_bar(
                out,
                [(prev[0] + next[0]) * 0.5, (prev[1] + next[1]) * 0.5],
                len,
                stroke,
                dy.atan2(dx),
                color,
            );
        }
        prev = next;
    }
}

fn emit_tool_ring(
    out: &mut Vec<RectInstance>,
    center: [f32; 2],
    radius: f32,
    segments: usize,
    stroke: f32,
    color: Color,
) {
    emit_tool_arc(
        out,
        center,
        [radius, radius],
        0.0,
        std::f32::consts::TAU,
        segments,
        stroke,
        color,
    );
}

fn emit_tool_eye_icon(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    hidden: bool,
    sf: f32,
) {
    let [x, y, w, h] = rect;
    let stroke = (1.25 * sf).max(1.0);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let eye_w = (w.min(h) * 0.72).max(14.0 * sf);
    let eye_h = eye_w * 0.34;
    let rx = eye_w * 0.5;
    let ry = eye_h * 0.5;
    let center = [cx, cy];
    emit_tool_arc(
        out,
        center,
        [rx, ry],
        std::f32::consts::PI,
        std::f32::consts::TAU,
        8,
        stroke,
        color,
    );
    emit_tool_arc(
        out,
        center,
        [rx, ry],
        0.0,
        std::f32::consts::PI,
        8,
        stroke,
        color,
    );
    let iris = (eye_w * 0.18).max(3.3 * sf);
    emit_tool_ring(out, center, iris, 12, stroke, color);
    if hidden {
        emit_tool_bar(
            out,
            [cx, cy],
            eye_w * 1.02,
            stroke * 1.18,
            std::f32::consts::FRAC_PI_4,
            color,
        );
    }
}

fn emit_tool_menu_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.55 * sf).max(1.0);
    let len = (w.min(h) * 0.52).max(10.0 * sf);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    for offset in [-4.2 * sf, 0.0, 4.2 * sf] {
        emit_tool_bar(out, [cx, cy + offset], len, stroke, 0.0, color);
    }
}

fn emit_tool_list_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let stroke = (1.4 * sf).max(1.0);
    let line_w = (w.min(h) * 0.42).max(8.0 * sf);
    let dot = (2.0 * sf).max(stroke * 1.2);
    let left = x + w * 0.34;
    let cy = y + h * 0.5;
    for offset in [-4.0 * sf, 0.0, 4.0 * sf] {
        out.push(inst_radii(
            [left - dot * 1.9, cy + offset - dot * 0.5, dot, dot],
            color,
            [dot * 0.5; 4],
        ));
        out.push(inst_radii(
            [left, cy + offset - stroke * 0.5, line_w, stroke],
            color,
            [stroke * 0.5; 4],
        ));
    }
}

fn emit_tool_filter_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let icon = w.min(h);
    let stroke = (1.35 * sf).max(1.0).min(icon * 0.095);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let size = (icon * 0.60).max(12.5 * sf);
    let left = cx - size * 0.5;
    let top = cy - size * 0.5;
    let right = cx + size * 0.5;
    let top_y = top + size * 0.12;
    let box_bottom_y = top + size * 0.29;
    let neck_y = top + size * 0.62;
    let stem_bottom_y = top + size * 0.89;
    let neck_left_x = cx - size * 0.105;
    let neck_right_x = cx + size * 0.105;
    let top_left_x = left + size * 0.06;
    let top_right_x = right - size * 0.06;
    let radius = stroke * 0.5;

    out.push(inst_radii(
        [top_left_x, top_y, top_right_x - top_left_x, stroke],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [
            top_left_x - stroke * 0.5,
            top_y,
            stroke,
            box_bottom_y - top_y,
        ],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [
            top_right_x - stroke * 0.5,
            top_y,
            stroke,
            box_bottom_y - top_y,
        ],
        color,
        [radius; 4],
    ));

    let left_diag_dx = neck_left_x - top_left_x;
    let left_diag_dy = neck_y - box_bottom_y;
    let left_diag_len = (left_diag_dx * left_diag_dx + left_diag_dy * left_diag_dy).sqrt();
    let right_diag_dx = top_right_x - neck_right_x;
    let right_diag_dy = neck_y - box_bottom_y;
    let right_diag_len = (right_diag_dx * right_diag_dx + right_diag_dy * right_diag_dy).sqrt();
    emit_tool_bar(
        out,
        [
            (top_left_x + neck_left_x) * 0.5,
            (box_bottom_y + neck_y) * 0.5,
        ],
        left_diag_len,
        stroke,
        left_diag_dy.atan2(left_diag_dx),
        color,
    );
    emit_tool_bar(
        out,
        [
            (top_right_x + neck_right_x) * 0.5,
            (box_bottom_y + neck_y) * 0.5,
        ],
        right_diag_len,
        stroke,
        (neck_y - box_bottom_y).atan2(neck_right_x - top_right_x),
        color,
    );

    out.push(inst_radii(
        [
            neck_left_x - stroke * 0.5,
            neck_y,
            stroke,
            stem_bottom_y - neck_y,
        ],
        color,
        [radius; 4],
    ));
    out.push(inst_radii(
        [
            neck_right_x - stroke * 0.5,
            neck_y,
            stroke,
            stem_bottom_y - neck_y - size * 0.08,
        ],
        color,
        [radius; 4],
    ));
    emit_tool_bar(
        out,
        [
            (neck_left_x + neck_right_x) * 0.5,
            stem_bottom_y - size * 0.04,
        ],
        neck_right_x - neck_left_x + size * 0.05,
        stroke,
        -0.32,
        color,
    );
}

fn emit_tool_sort_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let size = (5.5 * sf).max(3.5);
    let cx = x + w * 0.5;
    out.push(inst_rounded_triangle(
        [cx - size * 0.5, y + h * 0.36 - size * 0.5, size, size],
        color,
        true,
        (0.7 * sf).max(0.5),
    ));
    out.push(inst_rounded_triangle(
        [cx - size * 0.5, y + h * 0.64 - size * 0.5, size, size],
        color,
        false,
        (0.7 * sf).max(0.5),
    ));
}

fn emit_tool_history_icon(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    undo: bool,
    sf: f32,
) {
    let [x, y, w, h] = rect;
    let icon = w.min(h);
    let stroke = (1.65 * sf).max(1.15).min(icon * 0.11);
    let cx = x + w * 0.5;
    let cy = y + h * 0.54;
    let radius = (icon * 0.26).max(stroke * 4.2);
    let inner_ratio = ((radius - stroke) / radius).clamp(0.58, 0.86);
    let sweep = std::f32::consts::TAU * 0.79;
    let top_angle = -std::f32::consts::FRAC_PI_2;
    let start = if undo { top_angle } else { top_angle - sweep };
    let head = (icon * 0.17).max(stroke * 2.45);
    let head_y = cy - radius;

    out.push(inst_loading_spinner(
        [cx - radius, cy - radius, radius * 2.0, radius * 2.0],
        [0.0, 0.0, 0.0, 0.0],
        color,
        start,
        sweep,
        inner_ratio,
        1.0,
    ));
    if undo {
        emit_tool_arrow_head(out, [cx - radius * 0.12, head_y], head, "left", color, sf);
    } else {
        emit_tool_arrow_head(out, [cx + radius * 0.12, head_y], head, "right", color, sf);
    }
}

fn emit_tool_pause_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let bar_w = (2.6 * sf).max(1.5);
    let bar_h = (w.min(h) * 0.45).max(9.0 * sf);
    let gap = (3.3 * sf).max(2.0);
    let cx = x + w * 0.5;
    let top = y + (h - bar_h) * 0.5;
    let radius = (0.9 * sf).max(0.6);
    for left in [cx - gap * 0.5 - bar_w, cx + gap * 0.5] {
        out.push(inst_radii([left, top, bar_w, bar_h], color, [radius; 4]));
    }
}

fn emit_tool_stop_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let size = (w.min(h) * 0.42).max(8.0 * sf);
    let radius = (1.4 * sf).max(0.8);
    out.push(inst_radii(
        [x + (w - size) * 0.5, y + (h - size) * 0.5, size, size],
        color,
        [radius; 4],
    ));
}

fn emit_tool_save_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let size = (w.min(h) * 0.52).max(10.0 * sf);
    let left = x + (w - size) * 0.5;
    let top = y + (h - size) * 0.5;
    let stroke = (1.25 * sf).max(1.0);
    let radius = (1.2 * sf).max(0.75);
    out.push(inst_radii([left, top, size, size], color, [radius; 4]));
    out.push(inst_radii(
        [
            left + stroke * 1.5,
            top + stroke * 1.4,
            size * 0.46,
            stroke * 2.2,
        ],
        [0.0, 0.0, 0.0, color[3] * 0.55],
        [stroke * 0.4; 4],
    ));
    out.push(inst_radii(
        [
            left + stroke * 1.5,
            top + size - stroke * 3.0,
            size - stroke * 3.0,
            stroke * 1.6,
        ],
        [0.0, 0.0, 0.0, color[3] * 0.55],
        [stroke * 0.4; 4],
    ));
}

fn emit_tool_more_icon(out: &mut Vec<RectInstance>, rect: [f32; 4], color: Color, sf: f32) {
    let [x, y, w, h] = rect;
    let dot = (2.2 * sf).max(1.4);
    let gap = (4.2 * sf).max(dot * 1.6);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    for offset in [-gap, 0.0, gap] {
        out.push(inst_radii(
            [cx + offset - dot * 0.5, cy - dot * 0.5, dot, dot],
            color,
            [dot * 0.5; 4],
        ));
    }
}

fn dropdown_chevron_width_for_style(node: &WidgetNode, sf: f32) -> f32 {
    node.style
        .parts
        .parts
        .get("chevron")
        .and_then(|part| part.layout.width)
        .map(|width| width.max(1.0) * sf)
        .unwrap_or(DROPDOWN_CHEVRON_WIDTH_LP * sf)
}

fn emit_dropdown_chevron(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    open: bool,
    sf: f32,
) {
    emit_triangle_chevron(out, rect, color, open, sf, None);
}

fn emit_triangle_chevron(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    open: bool,
    sf: f32,
    clip: Option<Rect>,
) {
    let [x, y, w, h] = rect;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let mark_w = w.min(10.0 * sf).max(6.0 * sf);
    let mark_h = (mark_w * 0.64).max(4.0 * sf);
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    let radius = (1.1 * sf).max(0.75);
    let mark_rect = [cx - mark_w * 0.5, cy - mark_h * 0.5, mark_w, mark_h];
    if let Some(clip) = clip {
        let mark = Rect {
            x: mark_rect[0],
            y: mark_rect[1],
            w: mark_rect[2],
            h: mark_rect[3],
        };
        let Some(visible) = mark.intersect(clip) else {
            return;
        };
        let clip_bounds = [
            visible.x - mark.x,
            visible.y - mark.y,
            visible.x + visible.w - mark.x,
            visible.y + visible.h - mark.y,
        ];
        out.push(inst_rounded_triangle_clipped(
            mark_rect,
            color,
            open,
            radius,
            clip_bounds,
        ));
    } else {
        out.push(inst_rounded_triangle(mark_rect, color, open, radius));
    }
}

fn collapsible_indicator_width_for_style(node: &WidgetNode, sf: f32) -> f32 {
    node.style
        .parts
        .parts
        .get("indicator")
        .and_then(|part| part.layout.width)
        .map(|width| width.max(1.0) * sf)
        .unwrap_or(DROPDOWN_CHEVRON_WIDTH_LP * sf)
}

fn emit_collapsible_indicator(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    color: Color,
    expanded: bool,
    sf: f32,
    clip: Option<Rect>,
) {
    emit_triangle_chevron(out, rect, color, expanded, sf, clip);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelScrollbarAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PanelScrollbarAxisGeometry {
    pub track: Rect,
    pub thumb: Rect,
    pub max_scroll: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PanelScrollbarGeometry {
    pub horizontal: Option<PanelScrollbarAxisGeometry>,
    pub vertical: Option<PanelScrollbarAxisGeometry>,
}

pub(crate) fn panel_scrollbar_geometry(
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    rect: Rect,
) -> Option<PanelScrollbarGeometry> {
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
    let min_scroll = (SCROLLBAR_VISIBILITY_EPSILON_PX * sf).max(1.0);
    let has_horizontal = max_scroll_x > min_scroll;
    let has_vertical = max_scroll_y > min_scroll;
    if !has_horizontal && !has_vertical {
        return None;
    }
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return None;
    }
    let implicit_container_scrollbar = matches!(node.kind, WidgetKind::Panel | WidgetKind::Modal)
        && node.style.layout.overflow.is_none()
        && node.style.layout.overflow_x.is_none()
        && node.style.layout.overflow_y.is_none();
    if implicit_container_scrollbar && rect.h < IMPLICIT_PANEL_SCROLLBAR_MIN_SIZE_PX * sf {
        return None;
    }

    let scroll_x = layout
        .scroll_x
        .get(&node.id)
        .copied()
        .unwrap_or_else(|| state.container_scroll_x(&node.id, max_scroll_x));
    let scroll_y = layout
        .scroll_y
        .get(&node.id)
        .copied()
        .unwrap_or_else(|| state.container_scroll_y(&node.id, max_scroll_y));
    let visual = visual_for(node, state, theme);
    let border_w = visual.border_width.unwrap_or(BORDER_WIDTH_LP).max(0.0) * sf;
    let panel_radius_lp = visual.border_radius.unwrap_or(theme.radius * 0.5).max(0.0);
    let panel_radii = visual_radii(&visual, panel_radius_lp, sf);
    let title_inset = panel_scrollbar_title_inset(node, layout, theme, sf).min(rect.h.max(0.0));
    let viewport_h = (rect.h - title_inset).max(1.0);
    let viewport_w = rect.w.max(1.0);
    let track_thickness = scrollbar_part_width_px(node, "scrollbar-track", 4.0, sf).max(2.0);
    let thumb_thickness = scrollbar_part_width_px(
        node,
        "scrollbar-thumb",
        track_thickness / sf.max(0.0001),
        sf,
    )
    .max(2.0);
    let gutter_thickness = track_thickness.max(thumb_thickness);
    let gap = (4.0 * sf).max(2.0);
    let part_padding = base_part_style(&node.style, "scrollbar-track")
        .and_then(|part| part.layout.padding)
        .map(|padding| (padding.max(0.0) * sf).max(border_w));

    let mut geometry = PanelScrollbarGeometry::default();
    if has_vertical {
        let content_h = viewport_h + max_scroll_y;
        let right_radius = panel_radii[1].max(panel_radii[2]);
        let default_vertical_pad = (border_w + gap * 1.5).max(right_radius * 0.6);
        let vertical_pad = part_padding
            .map(|padding| padding.max(default_vertical_pad))
            .unwrap_or(default_vertical_pad);
        let default_right_pad = default_vertical_pad;
        let right_pad = part_padding
            .map(|padding| padding.max(default_right_pad))
            .unwrap_or(default_right_pad);
        let horizontal_reserve = if has_horizontal {
            gutter_thickness + gap
        } else {
            0.0
        };
        let gutter_x = rect.x + rect.w - right_pad - gutter_thickness;
        let track_x = gutter_x + (gutter_thickness - track_thickness) * 0.5;
        let track_y = rect.y + title_inset + vertical_pad;
        let track_bottom = rect.y + rect.h - vertical_pad - horizontal_reserve;
        let track_h = (track_bottom - track_y).max(1.0);
        if gutter_x >= rect.x && track_h >= SCROLLBAR_MIN_TRACK_LEN_PX * sf {
            let thumb_h = (track_h * (viewport_h / content_h).clamp(0.0, 1.0))
                .max(18.0 * sf)
                .min(track_h);
            let travel = (track_h - thumb_h).max(0.0);
            let thumb_y = track_y + travel * (scroll_y / max_scroll_y).clamp(0.0, 1.0);
            geometry.vertical = Some(PanelScrollbarAxisGeometry {
                track: Rect {
                    x: track_x,
                    y: track_y,
                    w: track_thickness,
                    h: track_h,
                },
                thumb: Rect {
                    x: gutter_x + (gutter_thickness - thumb_thickness) * 0.5,
                    y: thumb_y,
                    w: thumb_thickness,
                    h: thumb_h,
                },
                max_scroll: max_scroll_y,
            });
        }
    }

    if has_horizontal {
        let content_w = viewport_w + max_scroll_x;
        let bottom_radius = panel_radii[2].max(panel_radii[3]);
        let default_horizontal_pad = (border_w + gap * 1.5).max(bottom_radius * 0.6);
        let horizontal_pad = part_padding
            .map(|padding| padding.max(default_horizontal_pad))
            .unwrap_or(default_horizontal_pad);
        let default_bottom_pad = default_horizontal_pad;
        let bottom_pad = part_padding
            .map(|padding| padding.max(default_bottom_pad))
            .unwrap_or(default_bottom_pad);
        let vertical_reserve = if has_vertical {
            gutter_thickness + gap
        } else {
            0.0
        };
        let gutter_y = rect.y + rect.h - bottom_pad - gutter_thickness;
        let track_x = rect.x + horizontal_pad;
        let track_right = rect.x + rect.w - horizontal_pad - vertical_reserve;
        let track_y = gutter_y + (gutter_thickness - track_thickness) * 0.5;
        let track_w = (track_right - track_x).max(1.0);
        if gutter_y >= rect.y && track_w >= SCROLLBAR_MIN_TRACK_LEN_PX * sf {
            let thumb_w = (track_w * (viewport_w / content_w).clamp(0.0, 1.0))
                .max(18.0 * sf)
                .min(track_w);
            let travel = (track_w - thumb_w).max(0.0);
            let thumb_x = track_x + travel * (scroll_x / max_scroll_x).clamp(0.0, 1.0);
            geometry.horizontal = Some(PanelScrollbarAxisGeometry {
                track: Rect {
                    x: track_x,
                    y: track_y,
                    w: track_w,
                    h: track_thickness,
                },
                thumb: Rect {
                    x: thumb_x,
                    y: gutter_y + (gutter_thickness - thumb_thickness) * 0.5,
                    w: thumb_w,
                    h: thumb_thickness,
                },
                max_scroll: max_scroll_x,
            });
        }
    }

    if geometry.horizontal.is_some() || geometry.vertical.is_some() {
        Some(geometry)
    } else {
        None
    }
}

pub(crate) fn table_scrollbar_geometry(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    rect: Rect,
) -> Option<PanelScrollbarGeometry> {
    let table_state = state.table(&node.id)?;
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return None;
    }
    let metrics = table::metrics_for_node(node, theme, sf);
    let visible = table::visible(table_state, &rect, metrics);
    let max_scroll_x = table_state.columns.len().saturating_sub(1) as f32;
    let max_scroll_y = table_state.rows.saturating_sub(1) as f32;
    let has_horizontal = max_scroll_x > 0.0 && visible.col_count < table_state.columns.len();
    let has_vertical = max_scroll_y > 0.0 && visible.row_count < table_state.rows;
    if !has_horizontal && !has_vertical {
        return None;
    }

    let visual = visual_for(node, state, theme);
    let border_w = visual.border_width.unwrap_or(BORDER_WIDTH_LP).max(0.0) * sf;
    let radii = visual_radii(&visual, visual.border_radius.unwrap_or(theme.radius), sf);
    let track_thickness = scrollbar_part_width_px(node, "scrollbar-track", 4.0, sf).max(2.0);
    let thumb_thickness = scrollbar_part_width_px(
        node,
        "scrollbar-thumb",
        track_thickness / sf.max(0.0001),
        sf,
    )
    .max(2.0);
    let gutter_thickness = track_thickness.max(thumb_thickness);
    let gap = (4.0 * sf).max(2.0);
    let part_padding = base_part_style(&node.style, "scrollbar-track")
        .and_then(|part| part.layout.padding)
        .map(|padding| (padding.max(0.0) * sf).max(border_w));
    let default_pad = (border_w + gap * 1.25).max(radii.iter().copied().fold(0.0, f32::max) * 0.25);
    let pad = part_padding
        .map(|padding| padding.max(default_pad))
        .unwrap_or(default_pad);

    let mut geometry = PanelScrollbarGeometry::default();
    let vertical_reserve = if has_vertical {
        gutter_thickness + gap
    } else {
        0.0
    };
    let horizontal_reserve = if has_horizontal {
        gutter_thickness + gap
    } else {
        0.0
    };

    if has_vertical {
        let viewport_h = (rect.h - metrics.header_h).max(1.0);
        let content_h = table_state.rows.max(1) as f32 * metrics.row_h;
        let gutter_x = rect.x + rect.w - pad - gutter_thickness;
        let track_x = gutter_x + (gutter_thickness - track_thickness) * 0.5;
        let track_y = rect.y + metrics.header_h + pad;
        let track_bottom = rect.y + rect.h - pad - horizontal_reserve;
        let track_h = (track_bottom - track_y).max(1.0);
        if gutter_x >= rect.x && track_h >= SCROLLBAR_MIN_TRACK_LEN_PX * sf {
            let thumb_h = (track_h * (viewport_h / content_h).clamp(0.0, 1.0))
                .max(18.0 * sf)
                .min(track_h);
            let travel = (track_h - thumb_h).max(0.0);
            let thumb_y =
                track_y + travel * (table_state.scroll_row as f32 / max_scroll_y).clamp(0.0, 1.0);
            geometry.vertical = Some(PanelScrollbarAxisGeometry {
                track: Rect {
                    x: track_x,
                    y: track_y,
                    w: track_thickness,
                    h: track_h,
                },
                thumb: Rect {
                    x: gutter_x + (gutter_thickness - thumb_thickness) * 0.5,
                    y: thumb_y,
                    w: thumb_thickness,
                    h: thumb_h,
                },
                max_scroll: max_scroll_y,
            });
        }
    }

    if has_horizontal {
        let viewport_w = (rect.w - metrics.index_w).max(1.0);
        let content_w = table::total_column_width(table_state, metrics).max(viewport_w);
        let gutter_y = rect.y + rect.h - pad - gutter_thickness;
        let track_x = rect.x + metrics.index_w + pad;
        let track_right = rect.x + rect.w - pad - vertical_reserve;
        let track_y = gutter_y + (gutter_thickness - track_thickness) * 0.5;
        let track_w = (track_right - track_x).max(1.0);
        if gutter_y >= rect.y && track_w >= SCROLLBAR_MIN_TRACK_LEN_PX * sf {
            let thumb_w = (track_w * (viewport_w / content_w).clamp(0.0, 1.0))
                .max(18.0 * sf)
                .min(track_w);
            let travel = (track_w - thumb_w).max(0.0);
            let thumb_x =
                track_x + travel * (table_state.scroll_col as f32 / max_scroll_x).clamp(0.0, 1.0);
            geometry.horizontal = Some(PanelScrollbarAxisGeometry {
                track: Rect {
                    x: track_x,
                    y: track_y,
                    w: track_w,
                    h: track_thickness,
                },
                thumb: Rect {
                    x: thumb_x,
                    y: gutter_y + (gutter_thickness - thumb_thickness) * 0.5,
                    w: thumb_w,
                    h: thumb_thickness,
                },
                max_scroll: max_scroll_x,
            });
        }
    }

    if geometry.horizontal.is_some() || geometry.vertical.is_some() {
        Some(geometry)
    } else {
        None
    }
}

fn emit_panel_scrollbar(
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    out: &mut Vec<RectInstance>,
) {
    let [x, y, w, h] = rect;
    let Some(geometry) =
        panel_scrollbar_geometry(node, layout, state, theme, sf, Rect { x, y, w, h })
    else {
        return;
    };
    let track_visual = part_visual_for(node, state, "scrollbar-track");
    let thumb_visual = part_visual_for(node, state, "scrollbar-thumb");
    let track_fallback = widget_part_paint_fallback(node, "scrollbar-track", theme, state)
        .background
        .unwrap_or_else(|| with_alpha(mix(theme.surface, theme.muted_text, 0.25), 0.22));
    let thumb_fallback = widget_part_paint_fallback(node, "scrollbar-thumb", theme, state)
        .background
        .unwrap_or_else(|| with_alpha(mix(theme.surface_alt, theme.muted_text, 0.45), 0.58));

    if let Some(vertical) = geometry.vertical {
        emit_scrollbar_part_rect(
            out,
            rect_array(vertical.track),
            &track_visual,
            theme,
            track_fallback,
            [vertical.track.w * 0.5; 4],
            sf,
        );
        emit_scrollbar_part_rect(
            out,
            rect_array(vertical.thumb),
            &thumb_visual,
            theme,
            thumb_fallback,
            [vertical.thumb.w * 0.5; 4],
            sf,
        );
    }

    if let Some(horizontal) = geometry.horizontal {
        emit_scrollbar_part_rect(
            out,
            rect_array(horizontal.track),
            &track_visual,
            theme,
            track_fallback,
            [horizontal.track.h * 0.5; 4],
            sf,
        );
        emit_scrollbar_part_rect(
            out,
            rect_array(horizontal.thumb),
            &thumb_visual,
            theme,
            thumb_fallback,
            [horizontal.thumb.h * 0.5; 4],
            sf,
        );
    }
}

fn emit_table_scrollbar(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    rect: Rect,
    out: &mut Vec<RectInstance>,
) {
    let Some(geometry) = table_scrollbar_geometry(node, state, theme, sf, rect) else {
        return;
    };
    let track_visual = part_visual_for(node, state, "scrollbar-track");
    let thumb_visual = part_visual_for(node, state, "scrollbar-thumb");
    let track_fallback = widget_part_paint_fallback(node, "scrollbar-track", theme, state)
        .background
        .unwrap_or_else(|| with_alpha(mix(theme.surface, theme.muted_text, 0.25), 0.20));
    let thumb_fallback = widget_part_paint_fallback(node, "scrollbar-thumb", theme, state)
        .background
        .unwrap_or_else(|| with_alpha(mix(theme.surface_alt, theme.muted_text, 0.52), 0.68));

    if let Some(vertical) = geometry.vertical {
        emit_scrollbar_part_rect(
            out,
            rect_array(vertical.track),
            &track_visual,
            theme,
            track_fallback,
            [vertical.track.w * 0.5; 4],
            sf,
        );
        emit_scrollbar_part_rect(
            out,
            rect_array(vertical.thumb),
            &thumb_visual,
            theme,
            thumb_fallback,
            [vertical.thumb.w * 0.5; 4],
            sf,
        );
    }

    if let Some(horizontal) = geometry.horizontal {
        emit_scrollbar_part_rect(
            out,
            rect_array(horizontal.track),
            &track_visual,
            theme,
            track_fallback,
            [horizontal.track.h * 0.5; 4],
            sf,
        );
        emit_scrollbar_part_rect(
            out,
            rect_array(horizontal.thumb),
            &thumb_visual,
            theme,
            thumb_fallback,
            [horizontal.thumb.h * 0.5; 4],
            sf,
        );
    }
}

fn rect_array(rect: Rect) -> [f32; 4] {
    [rect.x, rect.y, rect.w, rect.h]
}

fn panel_scrollbar_title_inset(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
) -> f32 {
    titled_container_geometry(node, layout, sf, theme)
        .and_then(|geometry| {
            layout
                .rects
                .get(&node.id)
                .map(|rect| geometry.body_viewport.y - rect.y)
        })
        .unwrap_or(0.0)
}

fn scrollbar_part_width_px(node: &WidgetNode, part: &str, fallback_lp: f32, sf: f32) -> f32 {
    base_part_style(&node.style, part)
        .and_then(|part| part.layout.width)
        .unwrap_or(fallback_lp)
        .max(1.0)
        * sf
}

fn emit_scrollbar_part_rect(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    visual: &VisualStyle,
    theme: &Theme,
    fallback_color: Color,
    fallback_radii: [f32; 4],
    sf: f32,
) {
    let radii = visual_radii_with_fallback(visual, fallback_radii, sf);
    let paint = resolve_part_background_paint(visual, theme, fallback_color, sf);
    let border_w = visual
        .border_width
        .map(|width| width.max(0.0) * sf)
        .unwrap_or(0.0)
        .min(rect[2].min(rect[3]) * 0.5);
    if border_w > 0.0 {
        let border = resolve_color(&visual.border_color, theme)
            .map(|color| apply_opacity(color, visual.opacity))
            .unwrap_or_else(|| apply_opacity(theme.border, visual.opacity));
        emit_bordered_paint_rect_radii(out, rect, border, paint, radii, border_w);
    } else {
        emit_paint_rect_radii(out, rect, paint, radii);
    }
}

fn resolve_part_background_paint(
    visual: &VisualStyle,
    theme: &Theme,
    fallback: Color,
    sf: f32,
) -> FillPaint {
    if visual.background_paint.is_some() || visual.background.is_some() {
        resolve_background_paint(visual, theme, fallback, sf)
    } else {
        FillPaint::Solid(apply_opacity(fallback, visual.opacity))
    }
}

fn resolve_overlay_opacity(style: &NodeStyle, base_opacity: f32) -> f32 {
    (base_opacity
        * style
            .visual
            .opacity
            .unwrap_or_else(crate::style::native_fallback_opacity))
    .clamp(0.0, 1.0)
}

fn overlay_color(
    color: &Option<crate::style::ColorRef>,
    theme: &Theme,
    fallback: [f32; 4],
    opacity: f32,
) -> [f32; 4] {
    let mut color = resolve_color(color, theme).unwrap_or(fallback);
    color[3] *= opacity.clamp(0.0, 1.0);
    color
}

fn resolve_background_paint(
    visual: &VisualStyle,
    theme: &Theme,
    fallback: [f32; 4],
    sf: f32,
) -> FillPaint {
    if visual
        .background_paint
        .as_ref()
        .is_some_and(background_paint_contains_image)
    {
        // Managed image backgrounds and their fallback color are composed by
        // ImageRenderer before normal widget primitives. Keeping this fill
        // transparent preserves borders and child controls drawn afterward.
        return FillPaint::Solid([0.0, 0.0, 0.0, 0.0]);
    }
    match &visual.background_paint {
        Some(BackgroundPaint::Color(color)) => {
            FillPaint::Solid(apply_opacity(color.resolve(theme), visual.opacity))
        }
        Some(BackgroundPaint::Layers(layers)) if !layers.is_empty() => FillPaint::Layers(
            layers
                .iter()
                .map(|paint| resolve_background_paint_layer(paint, visual, theme, fallback, sf))
                .collect(),
        ),
        Some(paint) => resolve_background_paint_layer(paint, visual, theme, fallback, sf),
        None => FillPaint::Solid(
            resolve_color(&visual.background, theme)
                .map(|color| apply_opacity(color, visual.opacity))
                .unwrap_or(fallback),
        ),
    }
}

fn background_paint_contains_image(paint: &BackgroundPaint) -> bool {
    match paint {
        BackgroundPaint::Image(_) => true,
        BackgroundPaint::Layers(layers) => layers.iter().any(background_paint_contains_image),
        _ => false,
    }
}

fn resolve_background_paint_layer(
    paint: &BackgroundPaint,
    visual: &VisualStyle,
    theme: &Theme,
    fallback: [f32; 4],
    sf: f32,
) -> FillPaint {
    match paint {
        BackgroundPaint::Color(color) => {
            FillPaint::Solid(apply_opacity(color.resolve(theme), visual.opacity))
        }
        BackgroundPaint::LinearGradient(gradient) if gradient.stops.len() >= 2 => {
            FillPaint::LinearGradient {
                stops: resolve_gradient_stop_colors(&gradient.stops, theme, visual.opacity),
                repeating: gradient.repeating,
                scale_factor: sf,
                interpolation: gradient_interpolation_mode(visual.gradient_interpolation),
                angle_deg: gradient.angle_deg,
            }
        }
        BackgroundPaint::RadialGradient(gradient) if gradient.stops.len() >= 2 => {
            FillPaint::RadialGradient {
                stops: resolve_gradient_stop_colors(&gradient.stops, theme, visual.opacity),
                repeating: gradient.repeating,
                scale_factor: sf,
                interpolation: gradient_interpolation_mode(visual.gradient_interpolation),
                center: gradient.center,
            }
        }
        BackgroundPaint::BlobGradient(gradient) if !gradient.blobs.is_empty() => {
            let (colors, centers, radii, count) =
                resolve_blob_gradient(&gradient.blobs, theme, visual.opacity);
            FillPaint::BlobGradient {
                colors,
                centers,
                radii,
                count,
                interpolation: gradient_interpolation_mode(visual.gradient_interpolation),
            }
        }
        BackgroundPaint::MeshGradient(gradient) => FillPaint::MeshGradient {
            colors: [
                apply_opacity(gradient.top_left.resolve(theme), visual.opacity),
                apply_opacity(gradient.top_right.resolve(theme), visual.opacity),
                apply_opacity(gradient.bottom_left.resolve(theme), visual.opacity),
                apply_opacity(gradient.bottom_right.resolve(theme), visual.opacity),
            ],
            interpolation: gradient_interpolation_mode(visual.gradient_interpolation),
        },
        BackgroundPaint::Pattern(pattern) => FillPaint::Pattern {
            kind: pattern.kind,
            foreground: apply_opacity(pattern.foreground.resolve(theme), visual.opacity),
            background: apply_opacity(pattern.background.resolve(theme), visual.opacity),
            tile_size_px: pattern.tile_size * sf,
        },
        BackgroundPaint::Image(_) => FillPaint::Solid([0.0, 0.0, 0.0, 0.0]),
        BackgroundPaint::Layers(layers) if !layers.is_empty() => FillPaint::Layers(
            layers
                .iter()
                .map(|paint| resolve_background_paint_layer(paint, visual, theme, fallback, sf))
                .collect(),
        ),
        _ => FillPaint::Solid(
            resolve_color(&visual.background, theme)
                .map(|color| apply_opacity(color, visual.opacity))
                .unwrap_or(fallback),
        ),
    }
}

fn signed_gradient_stop_count(count: u32, repeating: bool) -> f32 {
    let count = count.max(2) as f32;
    if repeating {
        -count
    } else {
        count
    }
}

fn gradient_interpolation_mode(mode: Option<GradientInterpolation>) -> f32 {
    match mode.unwrap_or(GradientInterpolation::Srgb) {
        GradientInterpolation::Srgb => 0.0,
        GradientInterpolation::LinearSrgb => 1.0,
        GradientInterpolation::Oklab => 2.0,
    }
}

fn resolve_gradient_stop_colors(
    stops: &[crate::style::GradientStop],
    theme: &Theme,
    opacity: Option<f32>,
) -> Vec<ResolvedGradientStop> {
    stops
        .iter()
        .map(|stop| ResolvedGradientStop {
            color: apply_opacity(stop.color.resolve(theme), opacity),
            position: stop.position,
        })
        .collect()
}

fn prepare_gradient_stops(
    stops: &[ResolvedGradientStop],
    line_length_px: f32,
    scale_factor: f32,
) -> (
    [[f32; 4]; GRADIENT_STOP_CAPACITY],
    [f32; GRADIENT_STOP_CAPACITY],
    u32,
) {
    let resolved = normalize_gradient_stops(stops, line_length_px, scale_factor);
    if resolved.len() <= GRADIENT_STOP_CAPACITY {
        let mut colors = [[0.0, 0.0, 0.0, 0.0]; GRADIENT_STOP_CAPACITY];
        let mut positions = [1.0; GRADIENT_STOP_CAPACITY];
        positions[0] = 0.0;
        for (index, (color, position)) in resolved.iter().enumerate() {
            colors[index] = *color;
            positions[index] = *position;
        }
        let last = resolved
            .last()
            .map(|(color, _)| *color)
            .unwrap_or(colors[0]);
        for color in colors.iter_mut().skip(resolved.len()) {
            *color = last;
        }
        return (colors, positions, resolved.len().max(2) as u32);
    }

    let sample_positions = [0.0, 0.20, 0.40, 0.60, 0.80, 1.0];
    let mut colors = [[0.0, 0.0, 0.0, 0.0]; GRADIENT_STOP_CAPACITY];
    for (index, position) in sample_positions.iter().enumerate() {
        colors[index] = gradient_color_at(&resolved, *position);
    }
    (colors, sample_positions, GRADIENT_STOP_CAPACITY as u32)
}

fn resolve_blob_gradient(
    blobs: &[crate::style::BlobGradientStop],
    theme: &Theme,
    opacity: Option<f32>,
) -> ([[f32; 4]; 4], [[f32; 2]; 4], [f32; 4], f32) {
    let mut colors = [[0.0, 0.0, 0.0, 0.0]; 4];
    let mut centers = [[0.5, 0.5]; 4];
    let mut radii = [0.42; 4];
    let count = blobs.len().min(4);
    for (index, blob) in blobs.iter().take(4).enumerate() {
        colors[index] = apply_opacity(blob.color.resolve(theme), opacity);
        centers[index] = blob.center;
        radii[index] = blob.radius;
    }
    if count > 0 {
        for index in count..4 {
            colors[index] = colors[count - 1];
            centers[index] = centers[count - 1];
            radii[index] = radii[count - 1];
        }
    }
    (colors, centers, radii, count.max(1) as f32)
}

fn normalize_gradient_stops(
    stops: &[ResolvedGradientStop],
    line_length_px: f32,
    scale_factor: f32,
) -> Vec<([f32; 4], f32)> {
    let len = stops.len();
    let line_length_px = line_length_px.max(0.001);
    let mut positions: Vec<Option<f32>> = stops
        .iter()
        .map(|stop| {
            stop.position.map(|position| {
                (position.percent / 100.0 + position.px * scale_factor / line_length_px)
                    .clamp(0.0, 1.0)
            })
        })
        .collect();
    if len == 0 {
        return Vec::new();
    }
    positions[0] = Some(positions[0].unwrap_or(0.0));
    positions[len - 1] = Some(positions[len - 1].unwrap_or(1.0));

    let mut index = 0usize;
    while index < len {
        if positions[index].is_some() {
            index += 1;
            continue;
        }
        let start = index;
        while index < len && positions[index].is_none() {
            index += 1;
        }
        let previous = positions[start - 1].unwrap_or(0.0);
        let next = positions[index].unwrap_or(1.0);
        let span = (index - start + 1) as f32;
        for stop_index in start..index {
            let t = (stop_index - start + 1) as f32 / span;
            positions[stop_index] = Some(previous + (next - previous) * t);
        }
    }

    let mut previous = 0.0;
    stops
        .iter()
        .zip(positions)
        .map(|(stop, position)| {
            let position = position.unwrap_or(previous).max(previous).clamp(0.0, 1.0);
            previous = position;
            (stop.color, position)
        })
        .collect()
}

fn gradient_color_at(stops: &[([f32; 4], f32)], position: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let position = position.clamp(0.0, 1.0);
    if position <= stops[0].1 {
        return stops[0].0;
    }
    for pair in stops.windows(2) {
        let (left_color, left_pos) = pair[0];
        let (right_color, right_pos) = pair[1];
        if position <= right_pos {
            let span = (right_pos - left_pos).abs().max(0.0001);
            return mix_premultiplied_alpha(left_color, right_color, (position - left_pos) / span);
        }
    }
    stops.last().map(|(color, _)| *color).unwrap_or(stops[0].0)
}

fn emit_paint_rect_radii(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    paint: FillPaint,
    radii: [f32; 4],
) {
    match paint {
        FillPaint::Solid(color) => out.push(inst_radii(rect, color, radii)),
        FillPaint::Layers(layers) => {
            for layer in layers.iter().rev() {
                emit_paint_rect_radii(out, rect, layer.clone(), radii);
            }
        }
        FillPaint::LinearGradient {
            stops,
            repeating,
            scale_factor,
            interpolation,
            angle_deg,
        } => {
            let angle = angle_deg.to_radians();
            let line_length = (rect[2] * angle.sin().abs() + rect[3] * angle.cos().abs()).max(1.0);
            let (colors, positions, count) =
                prepare_gradient_stops(&stops, line_length, scale_factor);
            out.push(inst_linear_gradient(
                rect,
                colors,
                positions,
                signed_gradient_stop_count(count, repeating),
                interpolation,
                radii,
                angle_deg,
            ));
        }
        FillPaint::RadialGradient {
            stops,
            repeating,
            scale_factor,
            interpolation,
            center,
        } => {
            let center_px = [rect[2] * center[0], rect[3] * center[1]];
            let line_length = [
                [0.0, 0.0],
                [rect[2], 0.0],
                [0.0, rect[3]],
                [rect[2], rect[3]],
            ]
            .into_iter()
            .map(|corner| (corner[0] - center_px[0]).hypot(corner[1] - center_px[1]))
            .fold(1.0_f32, f32::max);
            let (colors, positions, count) =
                prepare_gradient_stops(&stops, line_length, scale_factor);
            out.push(inst_radial_gradient(
                rect,
                colors,
                positions,
                signed_gradient_stop_count(count, repeating),
                interpolation,
                radii,
                center,
            ));
        }
        FillPaint::BlobGradient {
            colors,
            centers,
            radii: blob_radii,
            count,
            interpolation,
        } => out.push(inst_blob_gradient(
            rect,
            colors,
            centers,
            blob_radii,
            count,
            interpolation,
            radii,
        )),
        FillPaint::MeshGradient {
            colors,
            interpolation,
        } => out.push(inst_mesh_gradient(rect, colors, interpolation, radii)),
        FillPaint::Pattern {
            kind,
            foreground,
            background,
            tile_size_px,
        } => out.push(inst_background_pattern(
            rect,
            foreground,
            background,
            kind,
            tile_size_px,
            radii,
        )),
    }
}

fn overlay_radius(style: &NodeStyle, fallback_lp: f32, sf: f32) -> f32 {
    style.visual.border_radius.unwrap_or(fallback_lp).max(0.0) * sf
}

fn visual_radii(visual: &VisualStyle, fallback_radius_lp: f32, sf: f32) -> [f32; 4] {
    visual
        .corner_radii
        .resolve(fallback_radius_lp.max(0.0))
        .map(|radius| (radius.max(0.0) * sf).max(0.0))
}

fn visual_radii_with_fallback(
    visual: &VisualStyle,
    fallback_radii_px: [f32; 4],
    sf: f32,
) -> [f32; 4] {
    let uniform = visual
        .border_radius
        .map(|radius| (radius.max(0.0) * sf).max(0.0));
    [
        visual
            .corner_radii
            .top_left
            .map(|radius| (radius.max(0.0) * sf).max(0.0))
            .or(uniform)
            .unwrap_or(fallback_radii_px[0]),
        visual
            .corner_radii
            .top_right
            .map(|radius| (radius.max(0.0) * sf).max(0.0))
            .or(uniform)
            .unwrap_or(fallback_radii_px[1]),
        visual
            .corner_radii
            .bottom_right
            .map(|radius| (radius.max(0.0) * sf).max(0.0))
            .or(uniform)
            .unwrap_or(fallback_radii_px[2]),
        visual
            .corner_radii
            .bottom_left
            .map(|radius| (radius.max(0.0) * sf).max(0.0))
            .or(uniform)
            .unwrap_or(fallback_radii_px[3]),
    ]
}

fn inset_rect(rect: [f32; 4], inset: f32) -> [f32; 4] {
    [
        rect[0] + inset,
        rect[1] + inset,
        (rect[2] - inset * 2.0).max(1.0),
        (rect[3] - inset * 2.0).max(1.0),
    ]
}

fn paint_interaction(node: &WidgetNode, state: &WidgetState) -> PaintInteraction {
    if state.is_disabled(&node.id) {
        PaintInteraction::Disabled
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        PaintInteraction::Pressed
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        PaintInteraction::Focused
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        PaintInteraction::Hovered
    } else {
        PaintInteraction::Resting
    }
}

fn widget_paint_fallback(
    node: &WidgetNode,
    theme: &Theme,
    state: &WidgetState,
) -> NativePaintFallback {
    native_widget_paint_fallback_with_level(
        node.kind,
        node.props.level.as_deref(),
        theme,
        paint_interaction(node, state),
    )
}

fn widget_part_paint_fallback(
    node: &WidgetNode,
    part: &str,
    theme: &Theme,
    state: &WidgetState,
) -> NativePaintFallback {
    let selected = match node.kind {
        WidgetKind::Selectable | WidgetKind::TreeNode => state.is_selectable_selected(&node.id),
        WidgetKind::Tab => state.is_active_tab(&node.id),
        WidgetKind::NavItem => state.is_active_nav_item(&node.id),
        _ => false,
    };
    native_widget_part_paint_fallback_with_selection(
        node.kind,
        part,
        theme,
        paint_interaction(node, state),
        state.checked.get(&node.id).copied().unwrap_or(false),
        selected,
    )
}

fn widget_part_paint_fallback_with_checked(
    node: &WidgetNode,
    part: &str,
    theme: &Theme,
    state: &WidgetState,
    checked: bool,
) -> NativePaintFallback {
    native_widget_part_paint_fallback(
        node.kind,
        part,
        theme,
        paint_interaction(node, state),
        checked,
    )
}

fn control_fill(node: &WidgetNode, theme: &Theme, state: &WidgetState) -> [f32; 4] {
    widget_paint_fallback(node, theme, state)
        .background
        .unwrap_or_else(|| match paint_interaction(node, state) {
            PaintInteraction::Disabled => mix(theme.surface_alt, theme.disabled, 0.28),
            PaintInteraction::Pressed => darken(theme.accent, 0.15),
            PaintInteraction::Hovered | PaintInteraction::Focused => {
                mix(theme.surface_alt, theme.accent, 0.20)
            }
            PaintInteraction::Resting => theme.surface_alt,
        })
}

fn control_border(node: &WidgetNode, theme: &Theme, state: &WidgetState) -> [f32; 4] {
    widget_paint_fallback(node, theme, state)
        .border_color
        .unwrap_or_else(|| match paint_interaction(node, state) {
            PaintInteraction::Disabled => mix(theme.border, theme.disabled, 0.45),
            PaintInteraction::Focused => theme.accent,
            PaintInteraction::Pressed => darken(theme.accent, 0.08),
            PaintInteraction::Hovered => mix(theme.border, theme.accent, 0.35),
            PaintInteraction::Resting => theme.border,
        })
}

fn emit_asymmetric_css_border(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    radii: [f32; 4],
    visual: &VisualStyle,
    fallback_color: [f32; 4],
    theme: &Theme,
    sf: f32,
) {
    let widths = visual.effective_border_widths().map(|width| width * sf);
    let styles = visual.resolved_border_styles();
    let uniform_color = resolve_color(&visual.border_color, theme);
    let colors = [
        resolve_color(&visual.border_top_color, theme),
        resolve_color(&visual.border_right_color, theme),
        resolve_color(&visual.border_bottom_color, theme),
        resolve_color(&visual.border_left_color, theme),
    ]
    .map(|color| {
        apply_opacity(
            color.or(uniform_color).unwrap_or(fallback_color),
            visual.opacity,
        )
    });
    let [x, y, w, h] = rect;

    if widths[3] > 0.0 {
        let edge_rect = [x, y, widths[3].min(w), h];
        let edge_radii = [radii[0], 0.0, 0.0, radii[3]];
        out.push(if styles[3] == BorderLineStyle::Solid {
            inst_radii(edge_rect, colors[3], edge_radii)
        } else {
            inst_patterned_border_strip(edge_rect, colors[3], edge_radii, false, styles[3])
        });
    }
    if widths[1] > 0.0 {
        let width = widths[1].min(w);
        let edge_rect = [x + w - width, y, width, h];
        let edge_radii = [0.0, radii[1], radii[2], 0.0];
        out.push(if styles[1] == BorderLineStyle::Solid {
            inst_radii(edge_rect, colors[1], edge_radii)
        } else {
            inst_patterned_border_strip(edge_rect, colors[1], edge_radii, false, styles[1])
        });
    }
    if widths[0] > 0.0 {
        let edge_rect = [x, y, w, widths[0].min(h)];
        let edge_radii = [radii[0], radii[1], 0.0, 0.0];
        out.push(if styles[0] == BorderLineStyle::Solid {
            inst_radii(edge_rect, colors[0], edge_radii)
        } else {
            inst_patterned_border_strip(edge_rect, colors[0], edge_radii, true, styles[0])
        });
    }
    if widths[2] > 0.0 {
        let height = widths[2].min(h);
        let edge_rect = [x, y + h - height, w, height];
        let edge_radii = [0.0, 0.0, radii[2], radii[3]];
        out.push(if styles[2] == BorderLineStyle::Solid {
            inst_radii(edge_rect, colors[2], edge_radii)
        } else {
            inst_patterned_border_strip(edge_rect, colors[2], edge_radii, true, styles[2])
        });
    }
}

fn emit_bordered_rect(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    border: [f32; 4],
    fill: [f32; 4],
    radius: f32,
    border_w: f32,
) {
    emit_bordered_rect_radii(out, rect, border, fill, [radius; 4], border_w);
}

fn emit_bordered_rect_radii(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    border: [f32; 4],
    fill: [f32; 4],
    radii: [f32; 4],
    border_w: f32,
) {
    let width = border_w.max(0.0);
    if width <= 0.0 {
        out.push(inst_radii(rect, fill, radii));
        return;
    }
    out.push(inst_radii(
        inset_rect(rect, width),
        fill,
        inset_radii(radii, width),
    ));
    out.push(inst_outline_ring_clipped(
        rect,
        border,
        radii,
        width,
        default_local_clip(rect),
    ));
}

fn emit_bordered_paint_rect_radii(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    border: [f32; 4],
    fill: FillPaint,
    radii: [f32; 4],
    border_w: f32,
) {
    let width = border_w.max(0.0);
    if width <= 0.0 {
        emit_paint_rect_radii(out, rect, fill, radii);
        return;
    }
    emit_paint_rect_radii(
        out,
        inset_rect(rect, width),
        fill,
        inset_radii(radii, width),
    );
    out.push(inst_outline_ring_clipped(
        rect,
        border,
        radii,
        width,
        default_local_clip(rect),
    ));
}

fn emit_underpainted_bordered_paint_rect_radii(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    border: [f32; 4],
    fill: FillPaint,
    radii: [f32; 4],
    border_w: f32,
) {
    let width = border_w.max(0.0);
    if width <= 0.0 {
        emit_paint_rect_radii(out, rect, fill, radii);
        return;
    }
    out.push(inst_radii(rect, border, radii));
    emit_paint_rect_radii(
        out,
        inset_rect(rect, width),
        fill,
        inset_radii(radii, width),
    );
}

fn emit_box_shadows(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    radii: [f32; 4],
    visual: &VisualStyle,
    theme: &Theme,
    sf: f32,
    clip: Option<Rect>,
) {
    let Some(shadows) = &visual.box_shadows else {
        return;
    };
    for shadow in shadows {
        if shadow.inset {
            continue;
        }
        let blur = (shadow.blur.max(0.0) * sf).max(0.0);
        let spread = shadow.spread * sf;
        if blur <= 0.0 && spread.abs() <= f32::EPSILON {
            continue;
        }
        let shape_w = rect[2] + spread * 2.0;
        let shape_h = rect[3] + spread * 2.0;
        if shape_w <= 0.0 || shape_h <= 0.0 {
            continue;
        }
        let mut color = shadow.color.resolve(theme);
        color = apply_opacity(color, visual.opacity);
        if color[3] <= 0.001 {
            continue;
        }
        let offset_x = shadow.offset_x * sf;
        let offset_y = shadow.offset_y * sf;
        let shape_rect = [
            rect[0] + offset_x - spread,
            rect[1] + offset_y - spread,
            shape_w,
            shape_h,
        ];
        let cover_rect = [
            shape_rect[0] - blur,
            shape_rect[1] - blur,
            shape_rect[2] + blur * 2.0,
            shape_rect[3] + blur * 2.0,
        ];
        if cover_rect[2] <= 0.0 || cover_rect[3] <= 0.0 {
            continue;
        }
        let Some(local_clip) = local_clip_for_rect(cover_rect, clip) else {
            continue;
        };
        out.push(inst_shadow_clipped(
            cover_rect,
            color,
            outset_radii(radii, spread),
            blur,
            local_clip,
        ));
    }
}

fn emit_inset_box_shadows(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    radii: [f32; 4],
    visual: &VisualStyle,
    theme: &Theme,
    sf: f32,
) {
    let Some(shadows) = &visual.box_shadows else {
        return;
    };
    for shadow in shadows {
        if !shadow.inset {
            continue;
        }
        let blur = (shadow.blur.max(0.0) * sf).max(1.0);
        let mut color = shadow.color.resolve(theme);
        color = apply_opacity(color, visual.opacity);
        if color[3] <= 0.001 {
            continue;
        }
        out.push(inst_inset_shadow(
            rect,
            color,
            radii,
            blur,
            [shadow.offset_x * sf, shadow.offset_y * sf],
            shadow.spread * sf,
        ));
    }
}

fn widget_supports_box_shadow(kind: WidgetKind) -> bool {
    !matches!(
        kind,
        WidgetKind::Window | WidgetKind::Modal | WidgetKind::Tooltip | WidgetKind::Spacer
    )
}

fn widget_supports_outline(kind: WidgetKind) -> bool {
    !matches!(
        kind,
        WidgetKind::Window | WidgetKind::Tooltip | WidgetKind::Spacer
    )
}

fn emit_outline(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    radii: [f32; 4],
    visual: &VisualStyle,
    theme: &Theme,
    sf: f32,
    clip: Option<Rect>,
) {
    let has_outline = visual.outline_width.is_some()
        || visual.outline_color.is_some()
        || visual.outline_style.is_some();
    if !has_outline {
        return;
    }
    let width = visual.outline_width.unwrap_or(1.0).max(0.0) * sf;
    if width <= 0.0 {
        return;
    }
    let mut color = resolve_color(&visual.outline_color, theme).unwrap_or(theme.focus);
    color = apply_opacity(color, visual.opacity);
    if color[3] <= 0.001 {
        return;
    }
    let offset = visual.outline_offset.unwrap_or(0.0).max(0.0) * sf;
    let pad = offset + width;
    let outer = [
        rect[0] - pad,
        rect[1] - pad,
        rect[2] + pad * 2.0,
        rect[3] + pad * 2.0,
    ];
    let outer_radii = outset_radii(radii, pad);
    let Some(local_clip) = local_clip_for_rect(outer, clip) else {
        return;
    };
    let style = visual.outline_style.unwrap_or(BorderLineStyle::Solid);
    if style != BorderLineStyle::None {
        out.push(if style == BorderLineStyle::Solid {
            inst_outline_ring_clipped(outer, color, outer_radii, width, local_clip)
        } else {
            inst_patterned_outline_ring_clipped(outer, color, outer_radii, width, local_clip, style)
        });
    }
}

fn emit_focus_ring_radii(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    rect: [f32; 4],
    radii: [f32; 4],
    out: &mut Vec<RectInstance>,
) {
    if state.focused.as_deref() == Some(node.id.as_str()) && !state.is_disabled(&node.id) {
        let width = FOCUS_RING_LP * sf;
        if width <= 0.0 {
            return;
        }
        let outer = [
            rect[0] - width,
            rect[1] - width,
            rect[2] + width * 2.0,
            rect[3] + width * 2.0,
        ];
        out.push(inst_outline_ring_clipped(
            outer,
            with_alpha(theme.focus, 0.60),
            outset_radii(radii, width),
            width,
            default_local_clip(outer),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_primitive_overlays(
    tree: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
    stylesheets: &StylesheetStore,
    media: DgMediaEnvironment,
    toasts: &[ToastOverlay],
    window_w: f32,
    window_h: f32,
    out: &mut Vec<RectInstance>,
    icon_geometry_cache: &mut IconGeometryCache,
) {
    emit_dropdown_overlays(tree, layout, theme, sf, state, out);
    emit_menu_overlays(tree, layout, theme, sf, state, out);
    emit_modal_overlays(
        tree,
        layout,
        theme,
        sf,
        state,
        caret_positions,
        out,
        icon_geometry_cache,
    );
    emit_tooltip_overlay(
        tree,
        layout,
        theme,
        sf,
        state,
        caret_positions,
        stylesheets,
        media,
        out,
    );
    emit_toast_overlays(
        toasts,
        theme,
        sf,
        stylesheets,
        media,
        window_w,
        window_h,
        out,
    );
    emit_drag_drop_overlay(state, theme, sf, window_w, window_h, out);
}

fn emit_drag_drop_overlay(
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    window_w: f32,
    window_h: f32,
    out: &mut Vec<RectInstance>,
) {
    let Some(pos) = state.drag_pos else {
        return;
    };
    if state.drag_source.is_none() {
        return;
    }
    if window_w <= 0.0 || window_h <= 0.0 {
        return;
    }

    let accent = if state.drag_hover_target.is_some() {
        theme.success
    } else {
        theme.accent
    };
    let offset = 14.0 * sf;
    let chip_w = 54.0 * sf;
    let chip_h = 28.0 * sf;
    let margin = 6.0 * sf;
    let mut x = pos[0] + offset;
    let mut y = pos[1] + offset;
    if x + chip_w + margin > window_w {
        x = (pos[0] - chip_w - offset).max(margin);
    }
    if y + chip_h + margin > window_h {
        y = (pos[1] - chip_h - offset).max(margin);
    }

    let ring_size = 12.0 * sf;
    let ring_rect = [
        pos[0] - ring_size * 0.5,
        pos[1] - ring_size * 0.5,
        ring_size,
        ring_size,
    ];
    out.push(inst_radii(
        ring_rect,
        with_alpha(accent, 0.16),
        [ring_size * 0.5; 4],
    ));
    out.push(inst_outline_ring_clipped(
        ring_rect,
        with_alpha(accent, 0.74),
        [ring_size * 0.5; 4],
        (1.5 * sf).max(1.0),
        default_local_clip(ring_rect),
    ));

    let chip_rect = [x, y, chip_w, chip_h];
    let radius = 9.0 * sf;
    let shadow_rect = [x + 2.0 * sf, y + 4.0 * sf, chip_w, chip_h];
    out.push(inst_radii(shadow_rect, [0.0, 0.0, 0.0, 0.24], [radius; 4]));
    out.push(inst_radii(
        chip_rect,
        with_alpha(mix(theme.surface_alt, accent, 0.20), 0.94),
        [radius; 4],
    ));
    out.push(inst_outline_ring_clipped(
        chip_rect,
        with_alpha(accent, 0.78),
        [radius; 4],
        (1.5 * sf).max(1.0),
        default_local_clip(chip_rect),
    ));

    let dot = 8.0 * sf;
    out.push(inst_radii(
        [x + 10.0 * sf, y + (chip_h - dot) * 0.5, dot, dot],
        with_alpha(accent, 0.96),
        [dot * 0.5; 4],
    ));

    let handle_x = x + 24.0 * sf;
    let handle_y = y + 8.0 * sf;
    let handle_w = 17.0 * sf;
    let handle_h = (2.0 * sf).max(1.0);
    let handle_gap = 5.0 * sf;
    let handle = with_alpha(theme.text, 0.72);
    for index in 0..3 {
        let rect = [
            handle_x,
            handle_y + index as f32 * handle_gap,
            handle_w,
            handle_h,
        ];
        out.push(inst_radii(rect, handle, [handle_h * 0.5; 4]));
    }
}

fn emit_rects(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
    out: &mut Vec<RectInstance>,
) {
    let mut icon_geometry_cache = IconGeometryCache::default();
    emit_rects_inner(
        node,
        layout,
        theme,
        sf,
        state,
        caret_positions,
        false,
        RenderContext::default(),
        out,
        None,
        &mut icon_geometry_cache,
    );
}

#[derive(Clone, Copy, Default)]
struct RenderContext {
    tab_body_start: bool,
    transformed_ancestor: bool,
}

fn transparent_tab_body_container(kind: WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::VLayout
            | WidgetKind::HLayout
            | WidgetKind::GridLayout
            | WidgetKind::FlowLayout
            | WidgetKind::ScrollArea
            | WidgetKind::Pane
            | WidgetKind::Spacer
    )
}

fn emit_rects_inner(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
    skip_open_modals: bool,
    context: RenderContext,
    out: &mut Vec<RectInstance>,
    mut base_leaf_ranges: Option<&mut HashMap<String, Range<usize>>>,
    icon_geometry_cache: &mut IconGeometryCache,
) {
    if node.kind == WidgetKind::Tooltip {
        return;
    }
    if node.kind == WidgetKind::Modal && !node.props.open.unwrap_or(false) {
        return;
    }
    if skip_open_modals && node.kind == WidgetKind::Modal {
        return;
    }
    let subtree_primitive_start = out.len();
    let mut subtree_transform = None;
    let mut subtree_paint_clip = None;
    if layout.visible_rect(&node.id).is_some() {
        let own_primitive_start = out.len();
        let Some(full_rect) = layout.rects.get(&node.id).copied() else {
            return;
        };
        let paint_clip = layout.paint_clip_rect(&node.id);
        subtree_paint_clip = paint_clip;
        let [x, y, w, h] = [full_rect.x, full_rect.y, full_rect.w, full_rect.h];
        let visual = visual_for(node, state, theme);
        let paint_fallback = widget_paint_fallback(node, theme, state);
        let side_border_overrides = visual.has_border_side_overrides();
        let uniform_border_style = visual.border_style.unwrap_or(BorderLineStyle::Solid);
        let patterned_uniform_border = !side_border_overrides
            && matches!(
                uniform_border_style,
                BorderLineStyle::Dotted | BorderLineStyle::Dashed | BorderLineStyle::Double
            );
        let custom_css_border = side_border_overrides || patterned_uniform_border;
        let border_w = if custom_css_border {
            0.0
        } else {
            visual
                .border_width
                .or(paint_fallback.border_width)
                .unwrap_or(BORDER_WIDTH_LP)
                .max(0.0)
                * sf
        };
        let radius_lp = visual
            .border_radius
            .or(paint_fallback.border_radius)
            .unwrap_or(theme.radius)
            .max(0.0);
        let radius = radius_lp * sf;
        let radii = visual_radii(&visual, radius_lp, sf);
        let tab_attached_panel = context.tab_body_start && node.kind == WidgetKind::Panel;
        let styled_bg =
            resolve_color(&visual.background, theme).map(|c| apply_opacity(c, visual.opacity));
        let styled_border =
            resolve_color(&visual.border_color, theme).map(|c| apply_opacity(c, visual.opacity));
        let styled_accent =
            resolve_color(&visual.accent, theme).map(|c| apply_opacity(c, visual.opacity));
        subtree_transform = paint_transform_for_node(node, visual.transform).map(|transform| {
            (
                transform,
                [
                    full_rect.x + full_rect.w * 0.5,
                    full_rect.y + full_rect.h * 0.5,
                ],
            )
        });
        if widget_supports_box_shadow(node.kind) {
            let shadow_radii = if tab_attached_panel {
                let panel_radius_lp = visual.border_radius.unwrap_or(theme.radius * 0.5).max(0.0);
                let mut panel_radii = visual_radii(&visual, panel_radius_lp, sf);
                panel_radii[0] = 0.0;
                panel_radii[1] = 0.0;
                panel_radii
            } else {
                radii
            };
            emit_box_shadows(
                out,
                [full_rect.x, full_rect.y, full_rect.w, full_rect.h],
                shadow_radii,
                &visual,
                theme,
                sf,
                paint_clip,
            );
        }
        match node.kind {
            WidgetKind::Panel => {
                let panel_radius_lp = visual
                    .border_radius
                    .or(paint_fallback.border_radius)
                    .unwrap_or(theme.radius * 0.5)
                    .max(0.0);
                let mut panel_radii = visual_radii(&visual, panel_radius_lp, sf);
                if tab_attached_panel {
                    panel_radii[0] = 0.0;
                    panel_radii[1] = 0.0;
                }
                let panel_fill = resolve_background_paint(
                    &visual,
                    theme,
                    paint_fallback.background.unwrap_or(theme.surface),
                    sf,
                );
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border
                        .or(paint_fallback.border_color)
                        .unwrap_or(theme.border),
                    panel_fill,
                    panel_radii,
                    border_w,
                );
                emit_titled_container_surface_parts(
                    out,
                    node,
                    layout,
                    state,
                    theme,
                    sf,
                    panel_radii,
                    border_w,
                );
                if part_style_active_for_state(node, state, "accent") {
                    let accent_visual = part_visual_for(node, state, "accent");
                    let accent_w_lp = node
                        .style
                        .parts
                        .parts
                        .get("accent")
                        .and_then(|part| part.layout.width)
                        .unwrap_or(PANEL_ACCENT_WIDTH_LP)
                        .max(0.0);
                    let inner_w = (w - border_w * 2.0).max(0.0);
                    let inner_h = (h - border_w * 2.0).max(0.0);
                    let accent_w = (accent_w_lp * sf).min(inner_w);
                    if accent_w > 0.0 && inner_h > 0.0 {
                        let accent_fill = resolve_color(&accent_visual.background, theme)
                            .or_else(|| resolve_color(&accent_visual.foreground, theme))
                            .map(|color| {
                                apply_opacity(color, accent_visual.opacity.or(visual.opacity))
                            })
                            .unwrap_or_else(|| styled_accent.unwrap_or(theme.accent));
                        let inner_radii = inset_radii(panel_radii, border_w);
                        out.push(inst_radii_clipped(
                            [x + border_w, y + border_w, inner_w, inner_h],
                            accent_fill,
                            inner_radii,
                            [-1.0, -1.0, accent_w, inner_h + 1.0],
                        ));
                    }
                }
            }

            WidgetKind::DragSource | WidgetKind::DropTarget => {
                let is_drop_hover = state.drag_hover_target.as_deref() == Some(node.id.as_str());
                let has_visual = visual.background.is_some()
                    || visual.background_paint.is_some()
                    || visual.foreground.is_some()
                    || visual.border_color.is_some()
                    || visual.border_width.is_some()
                    || is_drop_hover;
                if has_visual {
                    emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                    let fallback_fill = if is_drop_hover {
                        with_alpha(styled_accent.unwrap_or(theme.accent), 0.14)
                    } else {
                        [0.0, 0.0, 0.0, 0.0]
                    };
                    let fill = if visual.background_paint.is_some() {
                        resolve_background_paint(&visual, theme, fallback_fill, sf)
                    } else {
                        FillPaint::Solid(styled_bg.unwrap_or(fallback_fill))
                    };
                    let border_color = styled_border.unwrap_or_else(|| {
                        if is_drop_hover {
                            styled_accent.unwrap_or(theme.accent)
                        } else {
                            with_alpha(theme.border, 0.0)
                        }
                    });
                    emit_bordered_paint_rect_radii(
                        out,
                        [x, y, w, h],
                        border_color,
                        fill,
                        radii,
                        border_w,
                    );
                    if is_drop_hover {
                        out.push(inst_outline_ring_clipped(
                            [x, y, w, h],
                            with_alpha(styled_accent.unwrap_or(theme.accent), 0.72),
                            radii,
                            (2.0 * sf).max(1.0),
                            default_local_clip([x, y, w, h]),
                        ));
                    }
                }
            }

            WidgetKind::Collapsible => {
                let expanded = state.is_expanded(&node.id);
                let header_visual = part_visual_for(node, state, "header");
                let body_visual = part_visual_for(node, state, "body");
                let header_fallback = widget_part_paint_fallback(node, "header", theme, state);
                let header_h = collapsible_header_height_for_style(&node.style, theme, sf).min(h);
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(
                        &visual,
                        theme,
                        paint_fallback.background.unwrap_or(theme.surface),
                        sf,
                    )
                } else {
                    FillPaint::Solid(
                        styled_bg
                            .or(paint_fallback.background)
                            .unwrap_or(theme.surface),
                    )
                };
                let border_color =
                    styled_border.unwrap_or_else(|| control_border(node, theme, state));
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                if border_w > 0.0 {
                    emit_paint_rect_radii(
                        out,
                        inset_rect([x, y, w, h], border_w),
                        fill,
                        inset_radii(radii, border_w),
                    );
                } else {
                    emit_paint_rect_radii(out, [x, y, w, h], fill, radii);
                }
                let header_fill = resolve_color(&header_visual.background, theme)
                    .map(|color| apply_opacity(color, header_visual.opacity))
                    .or(header_fallback.background)
                    .unwrap_or_else(|| {
                        if state.pressed.as_deref() == Some(node.id.as_str()) {
                            mix(
                                theme.surface_alt,
                                styled_accent.unwrap_or(theme.accent),
                                0.24,
                            )
                        } else if state.hovered.as_deref() == Some(node.id.as_str())
                            || state.focused.as_deref() == Some(node.id.as_str())
                        {
                            mix(
                                theme.surface_alt,
                                styled_accent.unwrap_or(theme.accent),
                                0.14,
                            )
                        } else {
                            theme.surface_alt
                        }
                    });
                push_masked_rect(
                    out,
                    [x, y, w, h],
                    header_fill,
                    radii,
                    [
                        x + border_w,
                        y + border_w,
                        (w - border_w * 2.0).max(1.0),
                        (header_h - border_w).max(1.0),
                    ],
                );
                if expanded && h > header_h + border_w {
                    let body_fill = resolve_color(&body_visual.background, theme)
                        .map(|color| apply_opacity(color, body_visual.opacity));
                    if let Some(body_fill) = body_fill {
                        push_masked_rect(
                            out,
                            [x, y, w, h],
                            body_fill,
                            radii,
                            [
                                x + border_w,
                                y + header_h,
                                (w - border_w * 2.0).max(1.0),
                                (h - header_h - border_w).max(1.0),
                            ],
                        );
                    }
                    out.push(inst(
                        [
                            x + border_w,
                            y + header_h,
                            (w - border_w * 2.0).max(1.0),
                            border_w.max(1.0),
                        ],
                        resolve_color(&header_visual.border_color, theme)
                            .or(Some(border_color))
                            .unwrap_or(theme.border),
                        0.0,
                    ));
                }
                if border_w > 0.0 {
                    out.push(inst_outline_ring_clipped(
                        [x, y, w, h],
                        border_color,
                        radii,
                        border_w,
                        default_local_clip([x, y, w, h]),
                    ));
                }
                let full_rect = layout
                    .rects
                    .get(&node.id)
                    .copied()
                    .unwrap_or(Rect { x, y, w, h });
                let full_header_h =
                    collapsible_header_height_for_style(&node.style, theme, sf).min(full_rect.h);
                let indicator_w = collapsible_indicator_width_for_style(node, sf);
                let indicator_rect = [
                    full_rect.x + theme.spacing * sf,
                    full_rect.y,
                    indicator_w,
                    full_header_h,
                ];
                emit_collapsible_indicator(
                    out,
                    indicator_rect,
                    single_part_mark_color(
                        node,
                        state,
                        theme,
                        "indicator",
                        widget_part_paint_fallback(node, "indicator", theme, state)
                            .background
                            .unwrap_or(theme.muted_text),
                    ),
                    expanded,
                    sf,
                    layout.visible_rect(&node.id),
                );
            }

            WidgetKind::Modal => {
                let root = root_rect(layout).unwrap_or(Rect { x, y, w, h });
                let scrim_visual = part_visual_for(node, state, "scrim");
                let scrim_fallback = apply_opacity(
                    widget_part_paint_fallback(node, "scrim", theme, state)
                        .background
                        .unwrap_or([0.0, 0.0, 0.0, 0.52]),
                    scrim_visual.opacity,
                );
                emit_paint_rect_radii(
                    out,
                    [root.x, root.y, root.w, root.h],
                    resolve_background_paint(&scrim_visual, theme, scrim_fallback, sf),
                    [0.0; 4],
                );
                if visual.box_shadows.is_some() {
                    emit_box_shadows(out, [x, y, w, h], radii, &visual, theme, sf, paint_clip);
                } else {
                    let shadow = 6.0 * sf;
                    out.push(inst_radii(
                        [x + shadow, y + shadow, w, h],
                        [0.0, 0.0, 0.0, 0.35],
                        radii,
                    ));
                }
                if node
                    .props
                    .text
                    .as_deref()
                    .is_some_and(|text| !text.is_empty())
                {
                    let inner_x = x + border_w;
                    let inner_y = y + border_w;
                    let inner_w = (w - border_w * 2.0).max(1.0);
                    let inner_h = (h - border_w * 2.0).max(1.0);
                    let title_band_h = titled_container_geometry(node, layout, sf, theme)
                        .map(|geometry| geometry.title_band.h)
                        .unwrap_or(0.0)
                        .min(inner_h);
                    let border_color = styled_border
                        .or(paint_fallback.border_color)
                        .unwrap_or(theme.border);
                    if title_band_h > 0.0 {
                        if border_w > 0.0 {
                            out.push(inst_radii([x, y, w, h], border_color, radii));
                        }
                        let base_fill = resolve_color(&visual.background, theme)
                            .or(paint_fallback.background)
                            .unwrap_or(theme.surface);
                        let accent = resolve_color(&visual.accent, theme).unwrap_or(theme.accent);
                        let header_fill =
                            apply_opacity(mix(base_fill, accent, 0.16), visual.opacity);
                        let inner_radii = inset_radii(radii, border_w);
                        push_masked_rect(
                            out,
                            [inner_x, inner_y, inner_w, inner_h],
                            header_fill,
                            inner_radii,
                            [inner_x, inner_y, inner_w, title_band_h],
                        );
                        let body_h = (inner_h - title_band_h).max(0.0);
                        if body_h > 0.0 {
                            emit_paint_rect_radii(
                                out,
                                [inner_x, inner_y + title_band_h, inner_w, body_h],
                                resolve_background_paint(
                                    &visual,
                                    theme,
                                    paint_fallback.background.unwrap_or(theme.surface),
                                    sf,
                                ),
                                [0.0, 0.0, inner_radii[2], inner_radii[3]],
                            );
                        }
                        out.push(inst(
                            [inner_x, inner_y + title_band_h, inner_w, border_w.max(1.0)],
                            apply_opacity(mix(border_color, accent, 0.28), visual.opacity),
                            0.0,
                        ));
                    }
                } else {
                    let fill = resolve_background_paint(
                        &visual,
                        theme,
                        paint_fallback.background.unwrap_or(theme.surface),
                        sf,
                    );
                    emit_underpainted_bordered_paint_rect_radii(
                        out,
                        [x, y, w, h],
                        styled_border
                            .or(paint_fallback.border_color)
                            .unwrap_or(theme.border),
                        fill,
                        radii,
                        border_w,
                    );
                }
                emit_titled_container_surface_parts(
                    out, node, layout, state, theme, sf, radii, border_w,
                );
                if let Some(button) = modal_close_button_rect(node, layout, theme, sf) {
                    let button_radius = button[2].min(button[3]) * 0.5;
                    let bg = apply_opacity(mix(theme.surface, theme.text, 0.10), visual.opacity);
                    out.push(inst_radii(button, bg, [button_radius; 4]));
                    emit_tool_x_icon(
                        out,
                        button,
                        apply_opacity(theme.muted_text, visual.opacity),
                        sf,
                    );
                }
            }

            WidgetKind::Sidebar => {
                emit_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_background_paint(
                        &visual,
                        theme,
                        paint_fallback.background.unwrap_or(theme.surface),
                        sf,
                    ),
                    radii,
                );
                out.push(inst(
                    [x + w - border_w, y, border_w, h],
                    styled_border
                        .or(paint_fallback.border_color)
                        .unwrap_or(theme.border),
                    0.0,
                ));
                emit_titled_container_surface_parts(
                    out, node, layout, state, theme, sf, radii, border_w,
                );
            }

            WidgetKind::StatusBar => {
                emit_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_background_paint(
                        &visual,
                        theme,
                        paint_fallback.background.unwrap_or(theme.surface),
                        sf,
                    ),
                    radii,
                );
                out.push(inst(
                    [x, y, w, border_w],
                    styled_border
                        .or(paint_fallback.border_color)
                        .unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::Splitter => {
                let has_box_style = styled_bg.is_some()
                    || styled_border.is_some()
                    || visual.background_paint.is_some()
                    || visual.border_width.is_some();
                if has_box_style {
                    let splitter_border_w = visual
                        .border_width
                        .map(|width| (width.max(0.0) * sf).max(0.0))
                        .unwrap_or(0.0);
                    let fill = if visual.background_paint.is_some() {
                        resolve_background_paint(
                            &visual,
                            theme,
                            styled_bg.unwrap_or([0.0, 0.0, 0.0, 0.0]),
                            sf,
                        )
                    } else {
                        FillPaint::Solid(styled_bg.unwrap_or([0.0, 0.0, 0.0, 0.0]))
                    };
                    if splitter_border_w > 0.0 || styled_border.is_some() {
                        emit_bordered_paint_rect_radii(
                            out,
                            [x, y, w, h],
                            styled_border.unwrap_or(theme.border),
                            fill,
                            radii,
                            splitter_border_w.max(if styled_border.is_some() {
                                border_w
                            } else {
                                0.0
                            }),
                        );
                    } else {
                        emit_paint_rect_radii(out, [x, y, w, h], fill, radii);
                    }
                }
                emit_splitter_gutters(node, layout, theme, sf, state, [x, y, w, h], out);
            }

            WidgetKind::Pane => {
                let pane_visual = visual
                    .as_ref()
                    .clone()
                    .merged(&part_visual_for(node, state, "pane"));
                let pane_bg = resolve_color(&pane_visual.background, theme)
                    .map(|color| apply_opacity(color, pane_visual.opacity.or(visual.opacity)))
                    .or(styled_bg);
                let pane_border = resolve_color(&pane_visual.border_color, theme)
                    .map(|color| apply_opacity(color, pane_visual.opacity.or(visual.opacity)))
                    .or(styled_border);
                let pane_border_w = pane_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(0.0);
                let has_pane_style = pane_bg.is_some()
                    || pane_border.is_some()
                    || pane_visual.background_paint.is_some()
                    || pane_border_w > 0.0;
                if has_pane_style {
                    let fill = if pane_visual.background_paint.is_some() {
                        resolve_background_paint(
                            &pane_visual,
                            theme,
                            pane_bg.unwrap_or([0.0, 0.0, 0.0, 0.0]),
                            sf,
                        )
                    } else {
                        FillPaint::Solid(pane_bg.unwrap_or([0.0, 0.0, 0.0, 0.0]))
                    };
                    let pane_radii = visual_radii_with_fallback(&pane_visual, radii, sf);
                    if pane_border_w > 0.0 || pane_border.is_some() {
                        emit_bordered_paint_rect_radii(
                            out,
                            [x, y, w, h],
                            pane_border.unwrap_or(theme.border),
                            fill,
                            pane_radii,
                            pane_border_w.max(if pane_border.is_some() { border_w } else { 0.0 }),
                        );
                    } else {
                        emit_paint_rect_radii(out, [x, y, w, h], fill, pane_radii);
                    }
                }
            }

            WidgetKind::MenuBar => {
                emit_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_background_paint(
                        &visual,
                        theme,
                        paint_fallback.background.unwrap_or(theme.surface),
                        sf,
                    ),
                    radii,
                );
                out.push(inst(
                    [x, y + h - border_w, w, border_w],
                    styled_border
                        .or(paint_fallback.border_color)
                        .unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::Separator => {
                out.push(inst(
                    [x, y, w.max(border_w), h.max(border_w)],
                    styled_bg
                        .or(styled_border)
                        .or(paint_fallback.background)
                        .unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::Tabs => {
                let header_visual = part_visual_for(node, state, "header");
                let header_h = tabs_header_height_for_style(&node.style, theme, sf);
                if part_style_active_for_state(node, state, "header") {
                    let header_border_w = header_visual
                        .border_width
                        .map(|width| (width.max(0.0) * sf).max(0.0))
                        .unwrap_or(0.0);
                    let header_radii = visual_radii_with_fallback(&header_visual, [0.0; 4], sf);
                    let header_bg = resolve_color(&header_visual.background, theme)
                        .map(|color| apply_opacity(color, header_visual.opacity))
                        .or(styled_bg);
                    let has_header_fill =
                        header_bg.is_some() || header_visual.background_paint.is_some();
                    if has_header_fill {
                        let header_fill = if header_visual.background_paint.is_some() {
                            resolve_background_paint(
                                &header_visual,
                                theme,
                                header_bg.unwrap_or([0.0, 0.0, 0.0, 0.0]),
                                sf,
                            )
                        } else {
                            FillPaint::Solid(header_bg.unwrap_or([0.0, 0.0, 0.0, 0.0]))
                        };
                        emit_paint_rect_radii(out, [x, y, w, header_h], header_fill, header_radii);
                    }
                    if header_border_w > 0.0 || header_visual.border_color.is_some() {
                        out.push(inst(
                            [
                                x,
                                y + header_h - header_border_w.max(1.0),
                                w,
                                header_border_w.max(1.0),
                            ],
                            resolve_color(&header_visual.border_color, theme)
                                .or(styled_border)
                                .unwrap_or(theme.border),
                            0.0,
                        ));
                    }
                }
            }

            WidgetKind::Menu => {
                let menu_radius_lp = visual.border_radius.unwrap_or(4.0).max(0.0);
                let menu_radii = visual_radii(&visual, menu_radius_lp, sf);
                let menu_fill = visual
                    .background_paint
                    .as_ref()
                    .map(|_| resolve_background_paint(&visual, theme, theme.surface_alt, sf))
                    .or_else(|| styled_bg.map(FillPaint::Solid));
                let menu_border_w = visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(0.0);
                if menu_border_w > 0.0 {
                    let fill = menu_fill.unwrap_or(FillPaint::Solid([0.0, 0.0, 0.0, 0.0]));
                    emit_bordered_paint_rect_radii(
                        out,
                        [x, y, w, h],
                        styled_border.unwrap_or(theme.border),
                        fill,
                        menu_radii,
                        menu_border_w,
                    );
                } else if let Some(fill) = menu_fill {
                    emit_paint_rect_radii(out, [x, y, w, h], fill, menu_radii);
                }
            }

            WidgetKind::Button
            | WidgetKind::SmallButton
            | WidgetKind::IconButton
            | WidgetKind::ImageButton
            | WidgetKind::ArrowButton
            | WidgetKind::Dropdown => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let field_visual = (node.kind == WidgetKind::Dropdown)
                    .then(|| part_visual_for(node, state, "field"));
                let field_fill = field_visual
                    .as_ref()
                    .and_then(|field| resolve_color(&field.background, theme))
                    .map(|color| {
                        apply_opacity(color, field_visual.as_ref().and_then(|field| field.opacity))
                    });
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(
                        &visual,
                        theme,
                        paint_fallback.background.unwrap_or(theme.surface_alt),
                        sf,
                    )
                } else {
                    FillPaint::Solid(
                        field_fill
                            .or(styled_bg)
                            .or(paint_fallback.background)
                            .unwrap_or(theme.surface_alt),
                    )
                };
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    field_visual
                        .as_ref()
                        .and_then(|field| resolve_color(&field.border_color, theme))
                        .or(styled_border)
                        .or(paint_fallback.border_color)
                        .unwrap_or(theme.border),
                    fill,
                    radii,
                    border_w,
                );
                if matches!(node.kind, WidgetKind::Button | WidgetKind::SmallButton) {
                    if let Some(rect) =
                        badge_rect(node, [x, y, w, h], theme, sf, theme.spacing * sf)
                    {
                        emit_badge_pill(node, theme, sf, state, rect, out);
                    }
                } else if node.kind == WidgetKind::IconButton {
                    let icon_fallback = resolve_color(&visual.foreground, theme)
                        .map(|c| apply_opacity(c, visual.opacity))
                        .or_else(|| {
                            widget_part_paint_fallback(node, "icon", theme, state).background
                        })
                        .unwrap_or(theme.text);
                    let icon_color =
                        single_part_mark_color(node, state, theme, "icon", icon_fallback);
                    emit_tool_icon_button_mark(
                        out,
                        node,
                        [x, y, w, h],
                        icon_color,
                        sf,
                        icon_geometry_cache,
                    );
                } else if node.kind == WidgetKind::ArrowButton {
                    let icon_fallback = resolve_color(&visual.foreground, theme)
                        .map(|c| apply_opacity(c, visual.opacity))
                        .or_else(|| {
                            widget_part_paint_fallback(node, "icon", theme, state).background
                        })
                        .unwrap_or(theme.text);
                    let icon_color =
                        single_part_mark_color(node, state, theme, "icon", icon_fallback);
                    emit_arrow_button_mark(out, node, [x, y, w, h], icon_color, sf);
                } else if node.kind == WidgetKind::Dropdown {
                    let chevron_w = dropdown_chevron_width_for_style(node, sf);
                    let chevron_rect = [x + w - theme.spacing * sf - chevron_w, y, chevron_w, h];
                    emit_dropdown_chevron(
                        out,
                        chevron_rect,
                        single_part_mark_color(
                            node,
                            state,
                            theme,
                            "chevron",
                            widget_part_paint_fallback(node, "chevron", theme, state)
                                .background
                                .unwrap_or(theme.muted_text),
                        ),
                        state.open_dropdown.as_deref() == Some(node.id.as_str()),
                        sf,
                    );
                }
            }

            WidgetKind::Selectable => {
                let selected = state.is_selectable_selected(&node.id);
                let row_visual = visual
                    .as_ref()
                    .clone()
                    .merged(&part_visual_for(node, state, "row"));
                let row_fallback = widget_part_paint_fallback(node, "row", theme, state);
                let row_radii = visual_radii_with_fallback(&row_visual, radii, sf);
                let fallback_fill = if selected && styled_accent.is_some() {
                    mix(
                        theme.surface_alt,
                        styled_accent.unwrap_or(theme.accent),
                        0.24,
                    )
                } else {
                    row_fallback
                        .background
                        .or(paint_fallback.background)
                        .unwrap_or([0.0, 0.0, 0.0, 0.0])
                };
                let row_fill_solid = resolve_color(&row_visual.background, theme)
                    .map(|color| apply_opacity(color, row_visual.opacity))
                    .or(styled_bg)
                    .unwrap_or(fallback_fill);
                let row_fill = if row_visual.background_paint.is_some() {
                    resolve_background_paint(&row_visual, theme, row_fill_solid, sf)
                } else {
                    FillPaint::Solid(row_fill_solid)
                };
                let row_border_w = row_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(border_w);
                let row_border = resolve_color(&row_visual.border_color, theme)
                    .map(|color| apply_opacity(color, row_visual.opacity))
                    .or(styled_border)
                    .or(paint_fallback.border_color)
                    .unwrap_or([0.0, 0.0, 0.0, 0.0]);
                if row_border_w > 0.0 {
                    emit_bordered_paint_rect_radii(
                        out,
                        [x, y, w, h],
                        row_border,
                        row_fill,
                        row_radii,
                        row_border_w,
                    );
                } else if row_fill_solid[3] > 0.001 || row_visual.background_paint.is_some() {
                    emit_paint_rect_radii(out, [x, y, w, h], row_fill, row_radii);
                }
                if selected {
                    let indicator_visual = part_visual_for(node, state, "indicator");
                    let indicator_fallback =
                        widget_part_paint_fallback(node, "indicator", theme, state);
                    let indicator_style = node.style.parts.parts.get("indicator");
                    let indicator_w = indicator_style
                        .and_then(|style| style.layout.width)
                        .unwrap_or(3.0)
                        .max(1.0)
                        * sf;
                    let indicator_h = indicator_style
                        .and_then(|style| style.layout.height)
                        .unwrap_or(14.0)
                        .max(1.0)
                        * sf;
                    let indicator_x = x + (theme.spacing * 0.75 * sf).min(w.max(0.0));
                    let indicator_y = y + ((h - indicator_h) * 0.5).max(0.0);
                    let indicator_rect = [
                        indicator_x,
                        indicator_y,
                        indicator_w.min(w.max(1.0)),
                        indicator_h.min(h.max(1.0)),
                    ];
                    let indicator_color = apply_opacity(
                        resolve_color(&indicator_visual.background, theme)
                            .or(resolve_color(&indicator_visual.foreground, theme))
                            .unwrap_or_else(|| {
                                if styled_accent.is_some() {
                                    styled_accent.unwrap_or(theme.accent)
                                } else {
                                    indicator_fallback.background.unwrap_or(theme.accent)
                                }
                            }),
                        indicator_visual.opacity,
                    );
                    let indicator_radius =
                        (indicator_rect[2].min(indicator_rect[3]) * 0.5).max(0.0);
                    let indicator_radii =
                        visual_radii_with_fallback(&indicator_visual, [indicator_radius; 4], sf);
                    out.push(inst_radii(indicator_rect, indicator_color, indicator_radii));
                }
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], row_radii, out);
            }

            WidgetKind::TreeNode => {
                let selected = state.is_selectable_selected(&node.id);
                let expanded = state.is_expanded(&node.id) && !node.children.is_empty();
                let row_h = tree_node_row_height_for_style(node, theme, sf, Some(h))
                    .min(h)
                    .max(1.0);
                let row_rect = [x, y, w, row_h];
                let row_visual = visual
                    .as_ref()
                    .clone()
                    .merged(&part_visual_for(node, state, "row"));
                let row_fallback = widget_part_paint_fallback(node, "row", theme, state);
                let row_radii = visual_radii_with_fallback(&row_visual, radii, sf);
                let fallback_fill = if selected && styled_accent.is_some() {
                    mix(
                        theme.surface_alt,
                        styled_accent.unwrap_or(theme.accent),
                        0.24,
                    )
                } else {
                    row_fallback.background.unwrap_or([0.0, 0.0, 0.0, 0.0])
                };
                let row_fill_solid = resolve_color(&row_visual.background, theme)
                    .map(|color| apply_opacity(color, row_visual.opacity))
                    .or(styled_bg)
                    .unwrap_or(fallback_fill);
                let row_fill = if row_visual.background_paint.is_some() {
                    resolve_background_paint(&row_visual, theme, row_fill_solid, sf)
                } else {
                    FillPaint::Solid(row_fill_solid)
                };
                let row_border_w = row_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .or(row_fallback.border_width.map(|width| width * sf))
                    .unwrap_or(border_w);
                let row_border = resolve_color(&row_visual.border_color, theme)
                    .map(|color| apply_opacity(color, row_visual.opacity))
                    .or(styled_border)
                    .or(row_fallback.border_color)
                    .unwrap_or([0.0, 0.0, 0.0, 0.0]);
                if row_border_w > 0.0 {
                    emit_bordered_paint_rect_radii(
                        out,
                        row_rect,
                        row_border,
                        row_fill,
                        row_radii,
                        row_border_w,
                    );
                } else if row_fill_solid[3] > 0.001 || row_visual.background_paint.is_some() {
                    emit_paint_rect_radii(out, row_rect, row_fill, row_radii);
                }

                let indicator_w = collapsible_indicator_width_for_style(node, sf);
                let indicator_rect = [x + theme.spacing * 0.5 * sf, y, indicator_w, row_h];
                if !node.children.is_empty() {
                    let indicator_fallback =
                        widget_part_paint_fallback(node, "indicator", theme, state);
                    emit_collapsible_indicator(
                        out,
                        indicator_rect,
                        single_part_mark_color(
                            node,
                            state,
                            theme,
                            "indicator",
                            indicator_fallback.background.unwrap_or(theme.muted_text),
                        ),
                        expanded,
                        sf,
                        layout.visible_rect(&node.id),
                    );
                }
                if expanded && h > row_h {
                    let guide_visual = part_visual_for(node, state, "guide");
                    let guide_fallback = widget_part_paint_fallback(node, "guide", theme, state);
                    let guide_color = apply_opacity(
                        resolve_color(&guide_visual.background, theme)
                            .or(resolve_color(&guide_visual.foreground, theme))
                            .or(guide_fallback.background)
                            .unwrap_or(theme.border),
                        guide_visual.opacity,
                    );
                    let guide_w = guide_visual
                        .border_width
                        .or(guide_fallback.border_width)
                        .unwrap_or(1.0)
                        .max(1.0)
                        * sf;
                    let guide_x = indicator_rect[0] + indicator_rect[2] * 0.5 - guide_w * 0.5;
                    out.push(inst(
                        [guide_x, y + row_h, guide_w.max(1.0), (h - row_h).max(1.0)],
                        guide_color,
                        0.0,
                    ));
                }
                emit_focus_ring_radii(node, theme, sf, state, row_rect, row_radii, out);
            }

            WidgetKind::RadioButton => {
                let selected = state.is_selectable_selected(&node.id);
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);

                let row_fill = if visual.background_paint.is_some() {
                    resolve_background_paint(
                        &visual,
                        theme,
                        paint_fallback.background.unwrap_or([0.0, 0.0, 0.0, 0.0]),
                        sf,
                    )
                } else {
                    FillPaint::Solid(
                        styled_bg
                            .or(paint_fallback.background)
                            .unwrap_or([0.0, 0.0, 0.0, 0.0]),
                    )
                };
                let row_border = styled_border
                    .or(paint_fallback.border_color)
                    .unwrap_or([0.0, 0.0, 0.0, 0.0]);
                if border_w > 0.0 {
                    emit_bordered_paint_rect_radii(
                        out,
                        [x, y, w, h],
                        row_border,
                        row_fill,
                        radii,
                        border_w,
                    );
                }

                let indicator_visual = part_visual_for(node, state, "indicator");
                let indicator_fallback = widget_part_paint_fallback_with_checked(
                    node,
                    "indicator",
                    theme,
                    state,
                    selected,
                );
                let indicator_style = node.style.parts.parts.get("indicator");
                let indicator_w = indicator_style
                    .and_then(|style| style.layout.width)
                    .unwrap_or(14.0)
                    .max(1.0)
                    * sf;
                let indicator_h = indicator_style
                    .and_then(|style| style.layout.height)
                    .unwrap_or(indicator_w / sf)
                    .max(1.0)
                    * sf;
                let indicator_side = indicator_w.min(indicator_h).max(1.0);
                let indicator_x = x + theme.spacing * sf;
                let indicator_y = y + ((h - indicator_side) * 0.5).max(0.0);
                let indicator_rect = [indicator_x, indicator_y, indicator_side, indicator_side];
                let disabled = state.is_disabled(&node.id);
                let indicator_border_w = indicator_visual
                    .border_width
                    .or(indicator_fallback.border_width)
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or((1.5 * sf).max(1.0));
                let indicator_border = apply_opacity(
                    resolve_color(&indicator_visual.border_color, theme).unwrap_or_else(|| {
                        if !disabled && selected && styled_accent.is_some() {
                            styled_accent.unwrap_or(theme.accent)
                        } else {
                            indicator_fallback.border_color.unwrap_or(theme.border)
                        }
                    }),
                    indicator_visual.opacity,
                );
                let indicator_fill_solid = apply_opacity(
                    resolve_color(&indicator_visual.background, theme)
                        .or(indicator_fallback.background)
                        .unwrap_or(theme.surface),
                    indicator_visual.opacity,
                );
                let indicator_fill = if indicator_visual.background_paint.is_some() {
                    resolve_background_paint(&indicator_visual, theme, indicator_fill_solid, sf)
                } else {
                    FillPaint::Solid(indicator_fill_solid)
                };
                let indicator_radii =
                    visual_radii_with_fallback(&indicator_visual, [indicator_side * 0.5; 4], sf);
                emit_bordered_paint_rect_radii(
                    out,
                    indicator_rect,
                    indicator_border,
                    indicator_fill,
                    indicator_radii,
                    indicator_border_w,
                );

                if selected {
                    let dot_visual = part_visual_for(node, state, "dot");
                    let dot_fallback =
                        widget_part_paint_fallback_with_checked(node, "dot", theme, state, true);
                    let dot_style = node.style.parts.parts.get("dot");
                    let dot_side = dot_style
                        .and_then(|style| style.layout.width.or(style.layout.height))
                        .unwrap_or(6.0)
                        .max(1.0)
                        * sf;
                    let dot_side =
                        dot_side.min((indicator_side - indicator_border_w * 2.0).max(1.0));
                    let dot_rect = [
                        indicator_x + (indicator_side - dot_side) * 0.5,
                        indicator_y + (indicator_side - dot_side) * 0.5,
                        dot_side,
                        dot_side,
                    ];
                    let dot_fill = apply_opacity(
                        resolve_color(&dot_visual.background, theme)
                            .or(resolve_color(&dot_visual.foreground, theme))
                            .unwrap_or_else(|| {
                                if !disabled && styled_accent.is_some() {
                                    styled_accent.unwrap_or(theme.accent)
                                } else {
                                    dot_fallback.background.unwrap_or(theme.accent)
                                }
                            }),
                        dot_visual.opacity,
                    );
                    let dot_radii =
                        visual_radii_with_fallback(&dot_visual, [dot_side * 0.5; 4], sf);
                    out.push(inst_radii(dot_rect, dot_fill, dot_radii));
                }
            }

            WidgetKind::Badge | WidgetKind::Tag => {
                let fill_solid = styled_bg
                    .or(paint_fallback.background)
                    .unwrap_or(theme.accent);
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, fill_solid, sf)
                } else {
                    FillPaint::Solid(fill_solid)
                };
                let border_color = styled_border
                    .or(paint_fallback.border_color)
                    .unwrap_or(fill_solid);
                let badge_border_w = visual
                    .border_width
                    .or(paint_fallback.border_width)
                    .unwrap_or(0.0)
                    .max(0.0)
                    * sf;
                if badge_border_w > 0.0 {
                    emit_bordered_paint_rect_radii(
                        out,
                        [x, y, w, h],
                        border_color,
                        fill,
                        radii,
                        badge_border_w,
                    );
                } else {
                    emit_paint_rect_radii(out, [x, y, w, h], fill, radii);
                }
            }

            WidgetKind::Led => {
                let state_off = led_state_is_off(node);
                let base_color = node
                    .props
                    .led_color
                    .as_ref()
                    .map(|color| color.resolve(theme))
                    .unwrap_or_else(|| led_default_color(node, theme));
                let default_fill = styled_bg.unwrap_or(if state_off {
                    mix(base_color, theme.background, 0.28)
                } else {
                    base_color
                });

                let dot_part_visual = part_visual_for(node, state, "dot");
                let dot_visual = visual.as_ref().clone().merged(&dot_part_visual);
                let dot_style = node.style.parts.parts.get("dot");
                let fallback_side = w.min(h).max(1.0);
                let dot_w = dot_style
                    .and_then(|style| style.layout.width)
                    .map(|width| width.max(1.0) * sf)
                    .unwrap_or(fallback_side)
                    .min(w.max(1.0));
                let dot_h = dot_style
                    .and_then(|style| style.layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(dot_w)
                    .min(h.max(1.0));
                let dot_x = x + (w - dot_w) * 0.5;
                let dot_y = y + (h - dot_h) * 0.5;
                let dot_rect = [dot_x, dot_y, dot_w, dot_h];
                let dot_radius = dot_w.min(dot_h) * 0.5;
                let fill_solid = resolve_color(&dot_part_visual.background, theme)
                    .map(|color| apply_opacity(color, dot_part_visual.opacity.or(visual.opacity)))
                    .unwrap_or_else(|| {
                        apply_opacity(default_fill, dot_part_visual.opacity.or(visual.opacity))
                    });

                if !state_off && fill_solid[3] > 0.001 {
                    let glow_visual = part_visual_for(node, state, "glow");
                    let glow_style = node.style.parts.parts.get("glow");
                    let glow_pad = fallback_side * 0.22;
                    let glow_w = glow_style
                        .and_then(|style| style.layout.width)
                        .map(|width| width.max(1.0) * sf)
                        .unwrap_or(dot_w + glow_pad * 2.0)
                        .max(1.0);
                    let glow_h = glow_style
                        .and_then(|style| style.layout.height)
                        .map(|height| height.max(1.0) * sf)
                        .unwrap_or(dot_h + glow_pad * 2.0)
                        .max(1.0);
                    let glow_rect = [
                        dot_x + (dot_w - glow_w) * 0.5,
                        dot_y + (dot_h - glow_h) * 0.5,
                        glow_w,
                        glow_h,
                    ];
                    let glow_start = out.len();
                    let glow_color = resolve_color(&glow_visual.background, theme)
                        .or_else(|| resolve_color(&glow_visual.foreground, theme))
                        .unwrap_or(fill_solid);
                    let glow_alpha =
                        glow_visual.opacity.unwrap_or(0.16) * visual.opacity.unwrap_or(1.0);
                    let glow_color = with_alpha(glow_color, glow_color[3] * glow_alpha);
                    let glow_radii =
                        visual_radii_with_fallback(&glow_visual, [glow_w.min(glow_h) * 0.5; 4], sf);
                    if glow_visual.box_shadows.is_some() {
                        emit_box_shadows(
                            out,
                            glow_rect,
                            glow_radii,
                            &glow_visual,
                            theme,
                            sf,
                            paint_clip,
                        );
                        if glow_visual.background.is_some()
                            || glow_visual.background_paint.is_some()
                        {
                            let glow_fill =
                                resolve_part_background_paint(&glow_visual, theme, glow_color, sf);
                            emit_paint_rect_radii(out, glow_rect, glow_fill, glow_radii);
                        }
                    } else if glow_color[3] > 0.001 {
                        let glow_cover = [glow_rect[0], glow_rect[1], glow_rect[2], glow_rect[3]];
                        if let Some(local_clip) = local_clip_for_rect(glow_cover, paint_clip) {
                            out.push(inst_shadow_clipped(
                                glow_cover,
                                glow_color,
                                glow_radii,
                                4.0 * sf,
                                local_clip,
                            ));
                        }
                    }
                    apply_transform_to_instances(
                        &mut out[glow_start..],
                        glow_visual.transform,
                        sf,
                        [
                            glow_rect[0] + glow_rect[2] * 0.5,
                            glow_rect[1] + glow_rect[3] * 0.5,
                        ],
                    );
                }

                let dot_start = out.len();
                emit_box_shadows(
                    out,
                    dot_rect,
                    visual_radii_with_fallback(&dot_part_visual, [dot_radius; 4], sf),
                    &dot_part_visual,
                    theme,
                    sf,
                    paint_clip,
                );
                let fill = if dot_visual.background_paint.is_some() {
                    resolve_background_paint(&dot_visual, theme, fill_solid, sf)
                } else {
                    FillPaint::Solid(fill_solid)
                };
                let border_color = resolve_color(&dot_part_visual.border_color, theme)
                    .map(|color| apply_opacity(color, dot_part_visual.opacity.or(visual.opacity)))
                    .or(styled_border)
                    .unwrap_or_else(|| {
                        if state_off {
                            mix(theme.border, fill_solid, 0.35)
                        } else {
                            darken(fill_solid, 0.42)
                        }
                    });
                let dot_border_w = dot_part_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .or_else(|| visual.border_width.map(|width| width.max(0.0) * sf))
                    .unwrap_or(border_w)
                    .min(dot_w.min(dot_h) * 0.35);
                let dot_radii = visual_radii_with_fallback(&dot_visual, [dot_radius; 4], sf);
                emit_bordered_paint_rect_radii(
                    out,
                    dot_rect,
                    border_color,
                    fill,
                    dot_radii,
                    dot_border_w,
                );
                apply_transform_to_instances(
                    &mut out[dot_start..],
                    dot_part_visual.transform,
                    sf,
                    [dot_x + dot_w * 0.5, dot_y + dot_h * 0.5],
                );

                if !state_off && fallback_side >= 8.0 {
                    let highlight_visual = part_visual_for(node, state, "highlight");
                    let highlight_style = node.style.parts.parts.get("highlight");
                    let highlight_w = highlight_style
                        .and_then(|style| style.layout.width)
                        .map(|width| width.max(1.0) * sf)
                        .unwrap_or(dot_w * 0.34)
                        .min(dot_w);
                    let highlight_h = highlight_style
                        .and_then(|style| style.layout.height)
                        .map(|height| height.max(1.0) * sf)
                        .unwrap_or(dot_h * 0.22)
                        .min(dot_h);
                    let highlight = [
                        dot_x + dot_w * 0.24,
                        dot_y + dot_h * 0.18,
                        highlight_w,
                        highlight_h,
                    ];
                    let highlight_color = resolve_color(&highlight_visual.background, theme)
                        .or_else(|| resolve_color(&highlight_visual.foreground, theme))
                        .map(|color| {
                            apply_opacity(color, highlight_visual.opacity.or(visual.opacity))
                        })
                        .unwrap_or_else(|| {
                            apply_opacity(
                                [1.0, 1.0, 1.0, 0.34],
                                highlight_visual.opacity.or(visual.opacity),
                            )
                        });
                    if highlight_color[3] > 0.001 {
                        let highlight_start = out.len();
                        let highlight_radii = visual_radii_with_fallback(
                            &highlight_visual,
                            [highlight_h * 0.5; 4],
                            sf,
                        );
                        emit_paint_rect_radii(
                            out,
                            highlight,
                            resolve_part_background_paint(
                                &highlight_visual,
                                theme,
                                highlight_color,
                                sf,
                            ),
                            highlight_radii,
                        );
                        apply_transform_to_instances(
                            &mut out[highlight_start..],
                            highlight_visual.transform,
                            sf,
                            [
                                highlight[0] + highlight[2] * 0.5,
                                highlight[1] + highlight[3] * 0.5,
                            ],
                        );
                    }
                }
            }

            WidgetKind::Tab => {
                let active = state.is_active_tab(&node.id);
                let tab_visual = part_visual_for(node, state, "tab");
                let accent_visual = part_visual_for(node, state, "accent");
                let tab_fallback = widget_part_paint_fallback(node, "tab", theme, state);
                let accent_fallback = widget_part_paint_fallback(node, "accent", theme, state);
                let gap = TAB_GAP_LP * sf;
                let top = TAB_TOP_INSET_LP * sf;
                let bottom = if active {
                    0.0
                } else {
                    TAB_INACTIVE_BOTTOM_INSET_LP * sf
                };
                let vx = x + gap * 0.5;
                let vy = y + top;
                let vw = (w - gap).max(1.0);
                let vh = (h - top - bottom).max(1.0);
                let vr = radius.min(vh * 0.35);
                let tab_radii = visual_radii_with_fallback(&tab_visual, [vr, vr, 0.0, 0.0], sf);
                emit_focus_ring_radii(node, theme, sf, state, [vx, vy, vw, vh], tab_radii, out);
                let fill = if active {
                    resolve_color(&tab_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| match styled_accent {
                            Some(accent) => mix(theme.surface_alt, accent, 0.24),
                            None => tab_fallback.background.unwrap_or(theme.surface_alt),
                        })
                } else {
                    resolve_color(&tab_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| {
                            if styled_accent.is_some()
                                && matches!(
                                    paint_interaction(node, state),
                                    PaintInteraction::Hovered | PaintInteraction::Focused
                                )
                            {
                                mix(
                                    theme.surface_alt,
                                    styled_accent.unwrap_or(theme.accent),
                                    0.12,
                                )
                            } else {
                                tab_fallback.background.unwrap_or(theme.surface_alt)
                            }
                        })
                };
                let tab_border_w = tab_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .or(tab_fallback.border_width.map(|width| width * sf))
                    .unwrap_or(border_w);
                emit_bordered_rect_radii(
                    out,
                    [vx, vy, vw, vh],
                    resolve_color(&tab_visual.border_color, theme)
                        .or(styled_border)
                        .or_else(|| active.then_some(styled_accent).flatten())
                        .or(tab_fallback.border_color)
                        .unwrap_or(theme.border),
                    apply_opacity(fill, tab_visual.opacity),
                    tab_radii,
                    tab_border_w,
                );
                if active {
                    let bar_h = node
                        .style
                        .parts
                        .parts
                        .get("accent")
                        .and_then(|part| part.layout.height)
                        .map(|height| (height.max(1.0) * sf).max(1.0))
                        .unwrap_or(TAB_ACTIVE_BAR_LP * sf);
                    let accent_border_w = accent_visual
                        .border_width
                        .map(|width| (width.max(0.0) * sf).max(0.0))
                        .unwrap_or(0.0);
                    let accent_rect = [vx, vy + vh - bar_h, vw, bar_h];
                    let accent_fill = apply_opacity(
                        resolve_color(&accent_visual.background, theme)
                            .or(resolve_color(&accent_visual.foreground, theme))
                            .or(styled_accent)
                            .or(accent_fallback.background)
                            .unwrap_or(theme.accent),
                        accent_visual.opacity,
                    );
                    let accent_radii = visual_radii_with_fallback(&accent_visual, [0.0; 4], sf);
                    if accent_border_w > 0.0 {
                        emit_bordered_rect_radii(
                            out,
                            accent_rect,
                            resolve_color(&accent_visual.border_color, theme)
                                .unwrap_or(accent_fill),
                            accent_fill,
                            accent_radii,
                            accent_border_w,
                        );
                    } else {
                        out.push(inst_radii(
                            [
                                accent_rect[0],
                                accent_rect[1],
                                accent_rect[2],
                                accent_rect[3],
                            ],
                            accent_fill,
                            accent_radii,
                        ));
                    }
                }
                if let Some(rect) = badge_rect(node, [x, y, w, h], theme, sf, gap) {
                    emit_badge_pill(node, theme, sf, state, rect, out);
                }
            }

            WidgetKind::NavItem => {
                let active = state.is_active_nav_item(&node.id);
                let item_visual = part_visual_for(node, state, "item");
                let accent_visual = part_visual_for(node, state, "accent");
                let item_fallback = widget_part_paint_fallback(node, "item", theme, state);
                let accent_fallback = widget_part_paint_fallback(node, "accent", theme, state);
                let item_radii = visual_radii_with_fallback(&item_visual, radii, sf);
                let fill = if active {
                    resolve_color(&item_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| match styled_accent {
                            Some(accent) => mix(theme.surface_alt, accent, 0.20),
                            None => item_fallback.background.unwrap_or(theme.surface_alt),
                        })
                } else {
                    resolve_color(&item_visual.background, theme)
                        .or(styled_bg)
                        .or(item_fallback.background)
                        .unwrap_or_else(|| control_fill(node, theme, state))
                };
                let item_border_w = item_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(0.0);
                let item_fill = apply_opacity(fill, item_visual.opacity);

                // Resolve accent bar width through the state cascade so that
                // NavItem::accent { width: 0px } hides it and
                // NavItem:selected::accent { width: 4px } overrides the base.
                // No minimum clamp — 0.0 means no bar.
                let bar_w = if active {
                    let base_w = node
                        .style
                        .parts
                        .parts
                        .get("accent")
                        .and_then(|p| p.layout.width);
                    let selected_w =
                        selected_part_style_for_state(&node.style, &node.id, state, "accent")
                            .and_then(|p| p.layout.width);
                    let pseudo_w =
                        state_part_style_for_state(&node.style, &node.id, state, "accent")
                            .and_then(|p| p.layout.width);
                    (pseudo_w
                        .or(selected_w)
                        .or(base_w)
                        .unwrap_or(PANEL_ACCENT_WIDTH_LP)
                        * sf)
                        .max(0.0)
                } else {
                    0.0
                };

                let accent_border_w = accent_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(0.0);

                if bar_w > 0.0 {
                    if item_border_w > 0.0 {
                        emit_bordered_rect_radii(
                            out,
                            [x, y, w, h],
                            resolve_color(&item_visual.border_color, theme).unwrap_or(theme.border),
                            item_fill,
                            item_radii,
                            item_border_w,
                        );
                    } else {
                        out.push(inst_radii([x, y, w, h], item_fill, item_radii));
                    }

                    let accent_fill = apply_opacity(
                        resolve_color(&accent_visual.background, theme)
                            .or(resolve_color(&accent_visual.foreground, theme))
                            .or(styled_accent)
                            .or(accent_fallback.background)
                            .unwrap_or(theme.accent),
                        accent_visual.opacity,
                    );
                    // Left inset of the bar is CSS-controllable via the accent
                    // part's padding (cascaded through states), defaulting to
                    // 2px. NavItem::accent { padding: 6px } floats the bar in a
                    // wider gutter instead of hugging the left edge.
                    let accent_inset_lp: f32 = {
                        let base_inset = node
                            .style
                            .parts
                            .parts
                            .get("accent")
                            .and_then(|p| p.layout.padding);
                        let selected_inset =
                            selected_part_style_for_state(&node.style, &node.id, state, "accent")
                                .and_then(|p| p.layout.padding);
                        let pseudo_inset =
                            state_part_style_for_state(&node.style, &node.id, state, "accent")
                                .and_then(|p| p.layout.padding);
                        pseudo_inset
                            .or(selected_inset)
                            .or(base_inset)
                            .unwrap_or(2.0)
                    };
                    let accent_x_inset = (accent_inset_lp.max(0.0) * sf).min((w - bar_w).max(0.0));
                    let accent_y_inset = (6.0 * sf).min(h * 0.25).max(0.0);
                    let accent_h = (h - accent_y_inset * 2.0).max(0.0);
                    let accent_rect = [x + accent_x_inset, y + accent_y_inset, bar_w, accent_h];
                    let accent_radius = (bar_w.min(accent_h) * 0.5).max(0.0);
                    let accent_radii =
                        visual_radii_with_fallback(&accent_visual, [accent_radius; 4], sf);
                    if accent_border_w > 0.0 {
                        emit_bordered_rect_radii(
                            out,
                            accent_rect,
                            resolve_color(&accent_visual.border_color, theme)
                                .unwrap_or(accent_fill),
                            accent_fill,
                            accent_radii,
                            accent_border_w,
                        );
                    } else {
                        out.push(inst_radii(accent_rect, accent_fill, accent_radii));
                    }
                } else {
                    // No bar (inactive, or width: 0px): full-width item.
                    if item_border_w > 0.0 {
                        emit_bordered_rect_radii(
                            out,
                            [x, y, w, h],
                            resolve_color(&item_visual.border_color, theme).unwrap_or(theme.border),
                            item_fill,
                            item_radii,
                            item_border_w,
                        );
                    } else {
                        out.push(inst_radii([x, y, w, h], item_fill, item_radii));
                    }
                }
                if widget_raw_str(node, "icon").is_some() {
                    let compact = nav_item_uses_compact_icon(node, w, sf);
                    let icon_size = (18.0 * sf).min((h - 8.0 * sf).max(1.0));
                    let item_pad = node
                        .style
                        .parts
                        .parts
                        .get("item")
                        .and_then(|part| part.layout.padding)
                        .unwrap_or(theme.spacing)
                        .max(0.0)
                        * sf;
                    let icon_x = if compact {
                        x + (w - icon_size) * 0.5
                    } else {
                        x + item_pad
                    };
                    let icon_rect = [icon_x, y + (h - icon_size) * 0.5, icon_size, icon_size];
                    let icon_color = resolve_color(&item_visual.foreground, theme)
                        .or(resolve_color(&node.style.visual.foreground, theme))
                        .unwrap_or(theme.text);
                    emit_tool_icon_button_mark(
                        out,
                        node,
                        icon_rect,
                        icon_color,
                        sf,
                        icon_geometry_cache,
                    );
                }
                if nav_item_uses_compact_icon(node, w, sf) {
                    if node
                        .props
                        .badge
                        .as_deref()
                        .is_some_and(|badge| !badge.is_empty())
                    {
                        let dot = 8.0 * sf;
                        let inset = 5.0 * sf;
                        let badge_visual = part_visual_for(node, state, "badge");
                        let color = resolve_color(&badge_visual.background, theme)
                            .or(resolve_color(&badge_visual.foreground, theme))
                            .unwrap_or(theme.accent);
                        out.push(inst_radii(
                            [x + w - inset - dot, y + inset, dot, dot],
                            color,
                            [dot * 0.5; 4],
                        ));
                    }
                } else if let Some(rect) =
                    badge_rect(node, [x, y, w, h], theme, sf, theme.spacing * sf)
                {
                    emit_badge_pill(node, theme, sf, state, rect, out);
                }
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], item_radii, out);
            }

            WidgetKind::TextInput
            | WidgetKind::TextArea
            | WidgetKind::CodeEditor
            | WidgetKind::LogView => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let field_visual = (node.kind == WidgetKind::CodeEditor)
                    .then(|| part_visual_for(node, state, "field"));
                let fill_solid = field_visual
                    .as_ref()
                    .and_then(|field| resolve_color(&field.background, theme))
                    .map(|color| {
                        apply_opacity(color, field_visual.as_ref().and_then(|field| field.opacity))
                    })
                    .or(styled_bg)
                    .or(paint_fallback.background)
                    .unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55));
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, fill_solid, sf)
                } else {
                    FillPaint::Solid(fill_solid)
                };
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    field_visual
                        .as_ref()
                        .and_then(|field| resolve_color(&field.border_color, theme))
                        .or(styled_border)
                        .or(paint_fallback.border_color)
                        .unwrap_or_else(|| control_border(node, theme, state)),
                    fill,
                    radii,
                    border_w,
                );
                let gutter_w = if node.kind == WidgetKind::CodeEditor {
                    let gutter_w = code_editor_gutter_width_for_style(&node.style, sf)
                        .min((w - theme.spacing * sf * 2.0).max(1.0) * 0.5);
                    let gutter_visual = part_visual_for(node, state, "gutter");
                    let gutter_color = resolve_color(&gutter_visual.background, theme)
                        .map(|color| apply_opacity(color, gutter_visual.opacity))
                        .unwrap_or_else(|| mix(fill_solid, theme.surface_alt, 0.38));
                    let inner_h = (h - border_w * 2.0).max(1.0);
                    let gutter_radii = [
                        (radii[0] - border_w).max(0.0),
                        0.0,
                        0.0,
                        (radii[3] - border_w).max(0.0),
                    ];
                    out.push(inst_radii(
                        [x + border_w, y + border_w, gutter_w, inner_h],
                        gutter_color,
                        gutter_radii,
                    ));
                    let divider_color = resolve_color(&gutter_visual.border_color, theme)
                        .map(|color| apply_opacity(color, gutter_visual.opacity))
                        .unwrap_or_else(|| mix(theme.border, theme.surface_alt, 0.35));
                    out.push(inst(
                        [
                            x + border_w + gutter_w,
                            y + border_w,
                            border_w.max(1.0),
                            inner_h,
                        ],
                        divider_color,
                        0.0,
                    ));
                    gutter_w
                } else {
                    0.0
                };
                emit_text_selection_rects(
                    out,
                    node,
                    state,
                    theme,
                    sf,
                    [x, y, w, h],
                    border_w,
                    gutter_w,
                );
                if state.focused.as_deref() == Some(node.id.as_str())
                    && node.kind != WidgetKind::LogView
                    && !state.is_disabled(&node.id)
                {
                    let pad = theme.spacing * sf;
                    let text_left = x + pad + gutter_w;
                    let text_w = (w - pad * 2.0 - gutter_w).max(1.0);
                    let caret_xy =
                        caret_xy_for_node(text_left, text_w, &node.id, state, caret_positions);
                    let caret_font_size = node.style.text.font_size.unwrap_or(theme.font_size) * sf;
                    let caret_h = (caret_font_size + 5.0 * sf).min((h - border_w * 2.0).max(1.0));
                    let multiline =
                        matches!(node.kind, WidgetKind::TextArea | WidgetKind::CodeEditor);
                    let caret_y = if multiline {
                        y + pad + caret_xy[1]
                    } else {
                        y + (h - caret_h) * 0.5
                    };
                    let caret_visual = part_visual_for(node, state, "caret");
                    let caret_width = node
                        .style
                        .parts
                        .parts
                        .get("caret")
                        .and_then(|part| part.layout.width)
                        .map(|width| width.max(1.0) * sf)
                        .unwrap_or(CARET_WIDTH_LP * sf);
                    let caret_color = resolve_color(&caret_visual.background, theme)
                        .or_else(|| resolve_color(&caret_visual.foreground, theme))
                        .or_else(|| resolve_color(&caret_visual.border_color, theme))
                        .map(|color| apply_opacity(color, caret_visual.opacity))
                        .unwrap_or(theme.focus);
                    let visible_caret = !multiline
                        || (caret_y < y + h - border_w && caret_y + caret_h > y + border_w);
                    if visible_caret {
                        let caret_y = if multiline {
                            caret_y.clamp(y + border_w, y + h - border_w - caret_h)
                        } else {
                            caret_y
                        };
                        out.push(inst(
                            [caret_xy[0], caret_y, caret_width, caret_h],
                            caret_color,
                            0.0,
                        ));
                    }
                }
            }

            WidgetKind::NumberInput => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let invalid = state.number_is_invalid(&node.id);
                let field_visual = part_visual_for(node, state, "field");
                let fill_solid = styled_bg
                    .or(paint_fallback.background)
                    .unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55));
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, fill_solid, sf)
                } else {
                    FillPaint::Solid(fill_solid)
                };
                let control_rect = [x, y, w, h];
                let control_border = if invalid {
                    theme.danger
                } else {
                    styled_border
                        .or(paint_fallback.border_color)
                        .unwrap_or_else(|| control_border(node, theme, state))
                };
                let control_border_w = border_w.max(0.0);
                if control_border_w > 0.0 {
                    emit_paint_rect_radii(
                        out,
                        inset_rect(control_rect, control_border_w),
                        fill,
                        inset_radii(radii, control_border_w),
                    );
                } else {
                    emit_paint_rect_radii(out, control_rect, fill, radii);
                }
                let step_w = number_stepper_width_for_style(&node.style, w, sf);
                let left_step_x = x;
                let right_step_x = x + w - step_w;
                let field_rect = [
                    x + step_w + border_w,
                    y + border_w,
                    (w - step_w * 2.0 - border_w * 2.0).max(1.0),
                    (h - border_w * 2.0).max(1.0),
                ];
                let field_border_w = field_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(0.0);
                let field_has_border = field_border_w > 0.0 || field_visual.border_color.is_some();
                let field_fill = resolve_color(&field_visual.background, theme)
                    .map(|color| apply_opacity(color, field_visual.opacity))
                    .or_else(|| field_has_border.then_some(fill_solid));
                if let Some(field_fill) = field_fill {
                    let field_radii =
                        visual_radii_with_fallback(&field_visual, [0.0, 0.0, 0.0, 0.0], sf);
                    if field_border_w > 0.0 || field_visual.border_color.is_some() {
                        emit_bordered_rect_radii(
                            out,
                            field_rect,
                            resolve_color(&field_visual.border_color, theme)
                                .map(|color| apply_opacity(color, field_visual.opacity))
                                .unwrap_or(field_fill),
                            field_fill,
                            field_radii,
                            field_border_w.max(border_w),
                        );
                    } else {
                        out.push(inst_radii(field_rect, field_fill, field_radii));
                    }
                }
                let step_interaction = if state.is_disabled(&node.id) {
                    PaintInteraction::Disabled
                } else if state.focused.as_deref() == Some(node.id.as_str()) {
                    PaintInteraction::Focused
                } else if state.hovered.as_deref() == Some(node.id.as_str()) {
                    PaintInteraction::Hovered
                } else {
                    PaintInteraction::Resting
                };
                let step_fallback = native_widget_part_paint_fallback(
                    WidgetKind::NumberInput,
                    "stepper",
                    theme,
                    step_interaction,
                    false,
                );
                let step_fill = if !state.is_disabled(&node.id)
                    && (state.hovered.as_deref() == Some(node.id.as_str())
                        || state.focused.as_deref() == Some(node.id.as_str()))
                    && styled_accent.is_some()
                {
                    mix(
                        theme.surface_alt,
                        styled_accent.unwrap_or(theme.accent),
                        0.16,
                    )
                } else {
                    step_fallback.background.unwrap_or(theme.surface_alt)
                };
                let stepper_visual = part_visual_for(node, state, "stepper");
                let stepper_up_visual =
                    stepper_visual.merged(&part_visual_for(node, state, "stepper-up"));
                let stepper_down_visual =
                    stepper_visual.merged(&part_visual_for(node, state, "stepper-down"));
                let stepper_divider_visual = part_visual_for(node, state, "stepper-divider");
                let divider_part_active = part_style_active_for_state(node, state, "divider");
                let divider_visual = if divider_part_active {
                    part_visual_for(node, state, "divider")
                } else {
                    stepper_divider_visual.clone()
                };
                let step_up_fill = resolve_color(&stepper_up_visual.background, theme)
                    .map(|color| apply_opacity(color, stepper_up_visual.opacity))
                    .unwrap_or(step_fill);
                let step_down_fill = resolve_color(&stepper_down_visual.background, theme)
                    .map(|color| apply_opacity(color, stepper_down_visual.opacity))
                    .unwrap_or(step_fill);
                let divider_color = resolve_color(&divider_visual.background, theme)
                    .or_else(|| resolve_color(&divider_visual.border_color, theme))
                    .map(|color| apply_opacity(color, divider_visual.opacity))
                    .or_else(|| {
                        native_widget_part_paint_fallback(
                            WidgetKind::NumberInput,
                            "divider",
                            theme,
                            PaintInteraction::Resting,
                            false,
                        )
                        .background
                    })
                    .unwrap_or(theme.border);
                let stepper_divider_color =
                    resolve_color(&stepper_divider_visual.background, theme)
                        .or_else(|| resolve_color(&stepper_divider_visual.border_color, theme))
                        .map(|color| apply_opacity(color, stepper_divider_visual.opacity))
                        .or_else(|| {
                            native_widget_part_paint_fallback(
                                WidgetKind::NumberInput,
                                "stepper-divider",
                                theme,
                                PaintInteraction::Resting,
                                false,
                            )
                            .background
                        })
                        .unwrap_or(divider_color);
                let divider_w = divider_visual
                    .border_width
                    .or_else(|| {
                        let part_name = if divider_part_active {
                            "divider"
                        } else {
                            "stepper-divider"
                        };
                        node.style
                            .parts
                            .parts
                            .get(part_name)
                            .and_then(|part| part.layout.width.or(part.layout.height))
                    })
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(border_w)
                    .max(1.0);
                let step_up_radii = visual_radii_with_fallback(
                    &stepper_up_visual,
                    [
                        0.0,
                        (radii[1] - border_w).max(0.0),
                        (radii[2] - border_w).max(0.0),
                        0.0,
                    ],
                    sf,
                );
                let step_down_radii = visual_radii_with_fallback(
                    &stepper_down_visual,
                    [
                        (radii[0] - border_w).max(0.0),
                        0.0,
                        0.0,
                        (radii[3] - border_w).max(0.0),
                    ],
                    sf,
                );
                let step_inner_y = y + border_w;
                let step_inner_h = (h - border_w * 2.0).max(1.0);
                let step_up_rect = [
                    right_step_x,
                    step_inner_y,
                    (step_w - border_w).max(1.0),
                    step_inner_h,
                ];
                let step_down_rect = [
                    left_step_x + border_w,
                    step_inner_y,
                    (step_w - border_w).max(1.0),
                    step_inner_h,
                ];
                out.push(inst_radii(step_down_rect, step_down_fill, step_down_radii));
                out.push(inst_radii(step_up_rect, step_up_fill, step_up_radii));
                out.push(inst(
                    [
                        x + step_w - divider_w * 0.5,
                        y + border_w,
                        divider_w,
                        h - border_w * 2.0,
                    ],
                    divider_color,
                    0.0,
                ));
                out.push(inst(
                    [
                        x + w - step_w - divider_w * 0.5,
                        y + border_w,
                        divider_w,
                        h - border_w * 2.0,
                    ],
                    stepper_divider_color,
                    0.0,
                ));
                if control_border_w > 0.0 {
                    out.push(inst_outline_ring_clipped(
                        control_rect,
                        control_border,
                        radii,
                        control_border_w,
                        default_local_clip(control_rect),
                    ));
                }
                emit_stepper_mark(
                    out,
                    step_down_rect,
                    number_stepper_mark_color(node, state, theme, "stepper-down"),
                    false,
                    sf,
                );
                emit_stepper_mark(
                    out,
                    step_up_rect,
                    number_stepper_mark_color(node, state, theme, "stepper-up"),
                    true,
                    sf,
                );
                if state.focused.as_deref() == Some(node.id.as_str())
                    && !state.is_disabled(&node.id)
                {
                    emit_text_selection_rects(
                        out,
                        node,
                        state,
                        theme,
                        sf,
                        [x, y, w, h],
                        border_w,
                        0.0,
                    );
                    let pad = theme.spacing * sf;
                    let text_left = x + step_w + pad;
                    let text_w = (w - step_w * 2.0 - pad * 2.0).max(1.0);
                    let caret_x =
                        caret_xy_for_node(text_left, text_w, &node.id, state, caret_positions)[0];
                    let caret_font_size = node.style.text.font_size.unwrap_or(theme.font_size) * sf;
                    let caret_visual = part_visual_for(node, state, "caret");
                    let caret_w = node
                        .style
                        .parts
                        .parts
                        .get("caret")
                        .and_then(|part| part.layout.width)
                        .map(|width| (width.max(1.0) * sf).max(1.0))
                        .unwrap_or(CARET_WIDTH_LP * sf);
                    let caret_h = node
                        .style
                        .parts
                        .parts
                        .get("caret")
                        .and_then(|part| part.layout.height)
                        .map(|height| (height.max(1.0) * sf).max(1.0))
                        .unwrap_or_else(|| {
                            (caret_font_size + 5.0 * sf).min((h - border_w * 2.0).max(1.0))
                        });
                    let caret_color = resolve_color(&caret_visual.background, theme)
                        .or_else(|| resolve_color(&caret_visual.foreground, theme))
                        .or_else(|| resolve_color(&caret_visual.border_color, theme))
                        .map(|color| apply_opacity(color, caret_visual.opacity))
                        .unwrap_or(if invalid { theme.danger } else { theme.focus });
                    out.push(inst(
                        [caret_x, y + (h - caret_h) * 0.5, caret_w, caret_h],
                        caret_color,
                        0.0,
                    ));
                }
            }

            WidgetKind::DragNumber => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let field_visual = part_visual_for(node, state, "field");
                let base_fill = styled_bg
                    .or(paint_fallback.background)
                    .unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55));
                let field_fill_solid = resolve_color(&field_visual.background, theme)
                    .map(|color| apply_opacity(color, field_visual.opacity))
                    .unwrap_or(base_fill);
                let field_fill = if field_visual.background_paint.is_some() {
                    resolve_background_paint(&field_visual, theme, field_fill_solid, sf)
                } else if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, field_fill_solid, sf)
                } else {
                    FillPaint::Solid(field_fill_solid)
                };
                let field_border = resolve_color(&field_visual.border_color, theme)
                    .map(|color| apply_opacity(color, field_visual.opacity))
                    .or(styled_border)
                    .or(paint_fallback.border_color)
                    .unwrap_or_else(|| control_border(node, theme, state));
                let field_border_w = field_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(border_w);
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    field_border,
                    field_fill,
                    visual_radii_with_fallback(&field_visual, radii, sf),
                    field_border_w,
                );

                let grip_visual = part_visual_for(node, state, "grip");
                let grip_fallback = widget_part_paint_fallback(node, "grip", theme, state);
                let grip_style = node.style.parts.parts.get("grip");
                let grip_w = grip_style
                    .and_then(|part| part.layout.width)
                    .unwrap_or(16.0)
                    .max(1.0)
                    * sf;
                let dot_color = resolve_color(&grip_visual.background, theme)
                    .or_else(|| resolve_color(&grip_visual.foreground, theme))
                    .or_else(|| resolve_color(&grip_visual.border_color, theme))
                    .map(|color| apply_opacity(color, grip_visual.opacity))
                    .unwrap_or_else(|| {
                        if !state.is_disabled(&node.id) && styled_accent.is_some() {
                            mix(
                                theme.muted_text,
                                styled_accent.unwrap_or(theme.accent),
                                0.32,
                            )
                        } else {
                            grip_fallback.background.unwrap_or(theme.disabled)
                        }
                    });
                let grip_slot_w = grip_w.min((w - field_border_w * 2.0).max(1.0));
                let mark_h = grip_style
                    .and_then(|part| part.layout.height)
                    .unwrap_or(2.5)
                    .max(1.0)
                    * sf;
                let mark_w = (grip_slot_w * 0.46).clamp(4.0 * sf, 8.0 * sf);
                let inner_right = x + w - field_border_w.max(0.0) - 4.0 * sf;
                let grip_center_x = inner_right - grip_slot_w * 0.5;
                let grip_center_y = y + h * 0.5;
                let gap = (mark_h * 2.4).max(4.0 * sf);
                for offset in [-1.0_f32, 0.0, 1.0] {
                    let mark_rect = [
                        grip_center_x - mark_w * 0.5,
                        grip_center_y + offset * gap - mark_h * 0.5,
                        mark_w,
                        mark_h,
                    ];
                    out.push(inst_radii(mark_rect, dot_color, [mark_h * 0.5; 4]));
                }
            }

            WidgetKind::Checkbox => {
                let box_style = node.style.parts.parts.get("box");
                let box_w = box_style
                    .and_then(|style| style.layout.width)
                    .map(|width| width.max(1.0) * sf)
                    .unwrap_or(CHECKBOX_BOX_LP * sf)
                    .min(w.max(1.0));
                let box_h = box_style
                    .and_then(|style| style.layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(CHECKBOX_BOX_LP * sf)
                    .min(h.max(1.0));
                let box_x = x + CHECKBOX_LEFT_PAD_LP * sf;
                let box_y = y + (h - box_h) * 0.5;
                let checked = state.checked.get(&node.id).copied().unwrap_or(false);
                let disabled = state.is_disabled(&node.id);
                let row_visual = part_visual_for(node, state, "row");
                let row_fallback = widget_part_paint_fallback(node, "row", theme, state);
                let has_label = node
                    .props
                    .text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty());
                let interaction_rect = if has_label {
                    [x, y, w, h]
                } else {
                    let left = (box_x - CHECKBOX_LEFT_PAD_LP * sf).max(x);
                    let right = (box_x + box_w + CHECKBOX_LEFT_PAD_LP * sf).min(x + w);
                    if right > left {
                        [left, y, right - left, h]
                    } else {
                        [x, y, w, h]
                    }
                };
                if !state.is_disabled(&node.id)
                    && (state.hovered.as_deref() == Some(node.id.as_str())
                        || state.pressed.as_deref() == Some(node.id.as_str())
                        || state.focused.as_deref() == Some(node.id.as_str()))
                    || row_visual.background.is_some()
                {
                    let row_fill = resolve_color(&row_visual.background, theme)
                        .map(|color| apply_opacity(color, row_visual.opacity))
                        .or(row_fallback.background)
                        .unwrap_or([0.0, 0.0, 0.0, 0.0]);
                    out.push(inst_radii(
                        interaction_rect,
                        row_fill,
                        visual_radii_with_fallback(&row_visual, radii, sf),
                    ));
                }
                emit_focus_ring_radii(node, theme, sf, state, interaction_rect, radii, out);
                let box_visual = part_visual_for(node, state, "box");
                let box_fallback = widget_part_paint_fallback(node, "box", theme, state);
                let default_fill = if checked {
                    if disabled || state.pressed.as_deref() == Some(node.id.as_str()) {
                        box_fallback.background.unwrap_or(theme.accent)
                    } else {
                        styled_accent
                            .or(box_fallback.background)
                            .unwrap_or(theme.accent)
                    }
                } else {
                    styled_bg.or(box_fallback.background).unwrap_or_else(|| {
                        mix(theme.surface, control_fill(node, theme, state), 0.55)
                    })
                };
                let default_border = if checked {
                    if disabled {
                        box_fallback.border_color.unwrap_or(theme.disabled)
                    } else {
                        styled_border
                            .or(styled_accent)
                            .or(box_fallback.border_color)
                            .unwrap_or(theme.accent)
                    }
                } else {
                    styled_border
                        .or(box_fallback.border_color)
                        .unwrap_or_else(|| control_border(node, theme, state))
                };
                let box_border_w = box_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .or(box_fallback.border_width.map(|width| width * sf))
                    .unwrap_or(border_w);
                let box_radius = radius_lp.min((box_w.min(box_h) / sf) * 0.28);
                emit_bordered_rect_radii(
                    out,
                    [box_x, box_y, box_w, box_h],
                    resolve_color(&box_visual.border_color, theme)
                        .map(|color| apply_opacity(color, box_visual.opacity))
                        .unwrap_or(default_border),
                    resolve_color(&box_visual.background, theme)
                        .map(|color| apply_opacity(color, box_visual.opacity))
                        .unwrap_or(default_fill),
                    visual_radii_with_fallback(&box_visual, [box_radius * sf; 4], sf),
                    box_border_w,
                );
                if checked {
                    let indicator_visual = part_visual_for(node, state, "indicator");
                    let indicator_fallback =
                        widget_part_paint_fallback(node, "indicator", theme, state);
                    let indicator_style = node.style.parts.parts.get("indicator");
                    let default_marker_size = (box_w.min(box_h) * 0.42).max(3.0 * sf);
                    let marker_w = indicator_style
                        .and_then(|style| style.layout.width)
                        .map(|size| size.max(1.0) * sf)
                        .unwrap_or(default_marker_size)
                        .min(box_w);
                    let marker_h = indicator_style
                        .and_then(|style| style.layout.height)
                        .map(|size| size.max(1.0) * sf)
                        .unwrap_or(marker_w)
                        .min(box_h);
                    let marker_pad = ((box_h - marker_h) * 0.5).max(0.0);
                    let marker_x = if box_w > box_h * 1.2 {
                        box_x + box_w - marker_w - marker_pad
                    } else {
                        box_x + (box_w - marker_w) * 0.5
                    };
                    let marker_y = box_y + (box_h - marker_h) * 0.5;
                    let marker_color = resolve_color(&indicator_visual.background, theme)
                        .or_else(|| resolve_color(&indicator_visual.foreground, theme))
                        .map(|color| apply_opacity(color, indicator_visual.opacity))
                        .or(indicator_fallback.background)
                        .unwrap_or(theme.text);
                    out.push(inst_radii(
                        [marker_x, marker_y, marker_w, marker_h],
                        marker_color,
                        visual_radii_with_fallback(
                            &indicator_visual,
                            [marker_w.min(marker_h) * 0.5; 4],
                            sf,
                        ),
                    ));
                }
            }

            WidgetKind::ToggleSwitch => {
                let track_style = node.style.parts.parts.get("track");
                let thumb_style = node.style.parts.parts.get("thumb");
                let track_w = track_style
                    .and_then(|style| style.layout.width)
                    .map(|width| width.max(1.0) * sf)
                    .unwrap_or(TOGGLE_SWITCH_TRACK_WIDTH_LP * sf)
                    .min(w.max(1.0));
                let track_h = track_style
                    .and_then(|style| style.layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(TOGGLE_SWITCH_TRACK_HEIGHT_LP * sf)
                    .min(h.max(1.0));
                let label_left = node
                    .props
                    .raw_props
                    .get("label_position")
                    .and_then(|value| value.as_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("left"));
                let track_x = if label_left {
                    (x + w - CHECKBOX_LEFT_PAD_LP * sf - track_w).max(x)
                } else {
                    x + CHECKBOX_LEFT_PAD_LP * sf
                };
                let track_y = y + (h - track_h) * 0.5;
                let checked = state.checked.get(&node.id).copied().unwrap_or(false);
                let progress = state
                    .checked_t
                    .get(&node.id)
                    .copied()
                    .unwrap_or(if checked { 1.0 } else { 0.0 })
                    .clamp(0.0, 1.0);
                let disabled = state.is_disabled(&node.id);
                let interactive = !disabled
                    && (state.hovered.as_deref() == Some(node.id.as_str())
                        || state.pressed.as_deref() == Some(node.id.as_str())
                        || state.focused.as_deref() == Some(node.id.as_str()));
                let row_visual = part_visual_for(node, state, "row");
                let row_fallback = widget_part_paint_fallback(node, "row", theme, state);
                if interactive || row_visual.background.is_some() {
                    let row_fill = resolve_color(&row_visual.background, theme)
                        .map(|color| apply_opacity(color, row_visual.opacity))
                        .or(row_fallback.background)
                        .unwrap_or([0.0, 0.0, 0.0, 0.0]);
                    out.push(inst_radii(
                        [x, y, w, h],
                        row_fill,
                        visual_radii_with_fallback(&row_visual, radii, sf),
                    ));
                }
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);

                let track_visual = part_visual_for(node, state, "track");
                let off_fallback =
                    widget_part_paint_fallback_with_checked(node, "track", theme, state, false);
                let on_fallback =
                    widget_part_paint_fallback_with_checked(node, "track", theme, state, true);
                let default_off_fill = styled_bg
                    .or(off_fallback.background)
                    .unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.70));
                let default_on_fill =
                    if disabled || state.pressed.as_deref() == Some(node.id.as_str()) {
                        on_fallback.background.unwrap_or(theme.accent)
                    } else {
                        styled_accent
                            .or(on_fallback.background)
                            .unwrap_or(theme.accent)
                    };
                let default_track_fill = mix(default_off_fill, default_on_fill, progress);
                let default_track_border = if progress >= 0.5 {
                    if disabled {
                        on_fallback.border_color.unwrap_or(theme.disabled)
                    } else {
                        styled_border
                            .or(styled_accent)
                            .or(on_fallback.border_color)
                            .unwrap_or(theme.accent)
                    }
                } else {
                    styled_border
                        .or(off_fallback.border_color)
                        .unwrap_or_else(|| control_border(node, theme, state))
                };
                let track_border_w = track_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .or(off_fallback.border_width.map(|width| width * sf))
                    .unwrap_or(border_w);
                emit_bordered_rect_radii(
                    out,
                    [track_x, track_y, track_w, track_h],
                    resolve_color(&track_visual.border_color, theme)
                        .map(|color| apply_opacity(color, track_visual.opacity))
                        .unwrap_or(default_track_border),
                    resolve_color(&track_visual.background, theme)
                        .map(|color| apply_opacity(color, track_visual.opacity))
                        .unwrap_or(default_track_fill),
                    visual_radii_with_fallback(&track_visual, [track_h * 0.5; 4], sf),
                    track_border_w,
                );

                let thumb_visual = part_visual_for(node, state, "thumb");
                let thumb_fallback = widget_part_paint_fallback(node, "thumb", theme, state);
                let default_thumb_size = (track_h - 4.0 * sf)
                    .max(1.0)
                    .min(TOGGLE_SWITCH_THUMB_SIZE_LP * sf);
                let thumb_w = thumb_style
                    .and_then(|style| style.layout.width)
                    .map(|width| width.max(1.0) * sf)
                    .unwrap_or(default_thumb_size)
                    .min(track_w);
                let thumb_h = thumb_style
                    .and_then(|style| style.layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(default_thumb_size)
                    .min(track_h);
                let inset = ((track_h - thumb_h) * 0.5)
                    .max(track_border_w + 2.0 * sf)
                    .min((track_w - thumb_w).max(0.0) * 0.5);
                let travel = (track_w - thumb_w - inset * 2.0).max(0.0);
                let thumb_x = track_x + inset + travel * progress;
                let thumb_y = track_y + (track_h - thumb_h) * 0.5;
                let default_thumb_fill = thumb_fallback.background.unwrap_or(theme.text);
                let default_thumb_border = thumb_fallback.border_color.unwrap_or(theme.border);
                emit_bordered_rect_radii(
                    out,
                    [thumb_x, thumb_y, thumb_w, thumb_h],
                    resolve_color(&thumb_visual.border_color, theme)
                        .map(|color| apply_opacity(color, thumb_visual.opacity))
                        .unwrap_or(default_thumb_border),
                    resolve_color(&thumb_visual.background, theme)
                        .or_else(|| resolve_color(&thumb_visual.foreground, theme))
                        .map(|color| apply_opacity(color, thumb_visual.opacity))
                        .unwrap_or(default_thumb_fill),
                    visual_radii_with_fallback(&thumb_visual, [thumb_w.min(thumb_h) * 0.5; 4], sf),
                    thumb_visual
                        .border_width
                        .map(|width| width.max(0.0) * sf)
                        .or(thumb_fallback
                            .border_width
                            .map(|width| width * sf)
                            .map(|width| width.min(track_h * 0.12)))
                        .unwrap_or((1.0 * sf).min(track_h * 0.12)),
                );
            }

            WidgetKind::ProgressBar => {
                let track_visual = part_visual_for(node, state, "track");
                let fill_visual = part_visual_for(node, state, "fill");
                let track_fallback = widget_part_paint_fallback(node, "track", theme, state);
                let fill_fallback = widget_part_paint_fallback(node, "fill", theme, state);
                let default_track_fill = styled_bg
                    .or(track_fallback.background)
                    .unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.60));
                let track_fill = resolve_color(&track_visual.background, theme)
                    .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)))
                    .unwrap_or(default_track_fill);
                let track_border_w = track_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .unwrap_or(border_w);
                let track_radii = visual_radii_with_fallback(&track_visual, radii, sf);
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_color(&track_visual.border_color, theme)
                        .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)))
                        .or(styled_border)
                        .or(track_fallback.border_color)
                        .unwrap_or(theme.border),
                    track_fill,
                    track_radii,
                    track_border_w,
                );
                let inset = (track_border_w + 2.0 * sf).max(track_border_w);
                let inner = inset_rect([x, y, w, h], inset);
                let fill_h = node
                    .style
                    .parts
                    .parts
                    .get("fill")
                    .and_then(|part| part.layout.height)
                    .map(|height| (height.max(1.0) * sf).min(inner[3]))
                    .unwrap_or(inner[3]);
                let fill_y = inner[1] + (inner[3] - fill_h) * 0.5;
                let t = state.slider_t(&node.id);
                let fill_w = inner[2] * t;
                if fill_w > 0.5 {
                    let fill_color = resolve_color(&fill_visual.background, theme)
                        .or_else(|| resolve_color(&fill_visual.foreground, theme))
                        .map(|color| apply_opacity(color, fill_visual.opacity.or(visual.opacity)))
                        .or_else(|| {
                            if state.is_disabled(&node.id) {
                                fill_fallback.background
                            } else {
                                styled_accent.or(fill_fallback.background)
                            }
                        })
                        .unwrap_or(theme.accent);
                    // Default the fill's corners to be concentric with the
                    // track (outer radius minus the inset) so the inner bar
                    // follows the container's shape instead of always defaulting
                    // to a pill. Clamp to a pill at most; an explicit ::fill
                    // border-radius still wins via visual_radii_with_fallback.
                    let fill_cap = fill_h * 0.5;
                    let concentric_fill_radii = [
                        (track_radii[0] - inset).clamp(0.0, fill_cap),
                        (track_radii[1] - inset).clamp(0.0, fill_cap),
                        (track_radii[2] - inset).clamp(0.0, fill_cap),
                        (track_radii[3] - inset).clamp(0.0, fill_cap),
                    ];
                    out.push(inst_progress_fill(
                        [inner[0], fill_y, inner[2], fill_h],
                        fill_color,
                        visual_radii_with_fallback(&fill_visual, concentric_fill_radii, sf),
                        fill_w,
                    ));
                }
            }

            WidgetKind::LoadingSpinner => {
                let disabled = state.is_disabled(&node.id);
                let size_lp = loading_spinner_size_lp(node);
                let spinner_size = (size_lp * sf).max(2.0);
                let stroke = (loading_spinner_stroke_lp(node, size_lp) * sf)
                    .max(1.0)
                    .min(spinner_size * 0.36);
                let has_label = node
                    .props
                    .text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty());
                let spinner_x = if has_label {
                    x
                } else {
                    x + ((w - spinner_size) * 0.5).max(0.0)
                };
                let spinner_y = y + ((h - spinner_size) * 0.5).max(0.0);
                let cx = spinner_x + spinner_size * 0.5;
                let cy = spinner_y + spinner_size * 0.5;
                let outer_radius = spinner_size * 0.5;
                let inner_ratio =
                    ((outer_radius - stroke) / outer_radius.max(0.001)).clamp(0.0, 0.95);

                let track_visual = part_visual_for(node, state, "track");
                let arc_visual = part_visual_for(node, state, "arc");
                let track_fallback = widget_part_paint_fallback(node, "track", theme, state);
                let arc_fallback = widget_part_paint_fallback(node, "arc", theme, state);
                let track_authored = resolve_color(&track_visual.background, theme)
                    .or_else(|| resolve_color(&track_visual.foreground, theme))
                    .or_else(|| resolve_color(&track_visual.border_color, theme))
                    .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)));
                let mut track_color = track_authored.unwrap_or_else(|| {
                    track_fallback.background.unwrap_or_else(|| {
                        apply_opacity(mix(theme.border, theme.surface_alt, 0.35), Some(0.52))
                    })
                });
                let arc_authored = resolve_color(&arc_visual.background, theme)
                    .or_else(|| resolve_color(&arc_visual.foreground, theme))
                    .or_else(|| resolve_color(&arc_visual.accent, theme))
                    .map(|color| apply_opacity(color, arc_visual.opacity.or(visual.opacity)));
                let mut arc_color = arc_authored
                    .or_else(|| {
                        if disabled {
                            arc_fallback.background
                        } else {
                            styled_accent.or(arc_fallback.background)
                        }
                    })
                    .unwrap_or(theme.accent);
                if disabled && track_authored.is_some() {
                    track_color = apply_opacity(track_color, Some(0.68));
                }
                if disabled && arc_authored.is_some() {
                    arc_color = apply_opacity(arc_color, Some(0.66));
                }

                let phase = loading_spinner_phase(node, disabled);
                let arc_len = LOADING_SPINNER_TAU * 0.72;
                out.push(inst_loading_spinner(
                    [
                        cx - outer_radius,
                        cy - outer_radius,
                        outer_radius * 2.0,
                        outer_radius * 2.0,
                    ],
                    track_color,
                    arc_color,
                    phase,
                    arc_len,
                    inner_ratio,
                    0.0,
                ));
            }

            WidgetKind::Image | WidgetKind::HtmlReport => {
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    resolve_background_paint(&visual, theme, theme.surface_alt, sf),
                    radii,
                    border_w,
                );
            }

            WidgetKind::Extension => {
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, theme.surface_alt, sf)
                } else {
                    FillPaint::Solid(styled_bg.unwrap_or_else(|| {
                        apply_opacity(mix(theme.surface_alt, theme.background, 0.22), Some(0.86))
                    }))
                };
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    fill,
                    radii,
                    border_w,
                );
                emit_extension_display_list(out, node, theme, [x, y, w, h]);
            }

            WidgetKind::Slider => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let track_visual = part_visual_for(node, state, "track");
                let fill_visual = part_visual_for(node, state, "fill");
                let thumb_visual = part_visual_for(node, state, "thumb");
                let track_fallback = widget_part_paint_fallback(node, "track", theme, state);
                let fill_fallback = widget_part_paint_fallback(node, "fill", theme, state);
                let thumb_fallback = widget_part_paint_fallback(node, "thumb", theme, state);
                let track_color = resolve_color(&track_visual.background, theme)
                    .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)))
                    .or_else(|| {
                        resolve_color(&visual.track_color, theme)
                            .map(|color| apply_opacity(color, visual.opacity))
                    })
                    .or(paint_fallback.track_color)
                    .or(track_fallback.background)
                    .unwrap_or(theme.border);
                let thumb_color = resolve_color(&thumb_visual.background, theme)
                    .or_else(|| resolve_color(&thumb_visual.foreground, theme))
                    .map(|color| apply_opacity(color, thumb_visual.opacity.or(visual.opacity)))
                    .or_else(|| {
                        resolve_color(&visual.thumb_color, theme)
                            .map(|color| apply_opacity(color, visual.opacity))
                    })
                    .or(styled_accent)
                    .or(paint_fallback.thumb_color)
                    .or(thumb_fallback.background)
                    .unwrap_or(theme.accent);
                let track_h = node
                    .style
                    .parts
                    .parts
                    .get("track")
                    .and_then(|part| part.layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(SLIDER_TRACK_HEIGHT_LP * sf)
                    .max(border_w);
                let track_y = y + (h - track_h) * 0.5;
                let margin = SLIDER_TRACK_MARGIN_LP * sf;
                let track_w = (w - 2.0 * margin).max(0.0);
                let track_rect = [x + margin, track_y, track_w, track_h];
                let track_radii = visual_radii_with_fallback(&track_visual, [track_h * 0.5; 4], sf);
                let track_border_w = track_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .unwrap_or(0.0);
                if track_border_w > 0.0 || track_visual.border_color.is_some() {
                    emit_bordered_rect_radii(
                        out,
                        track_rect,
                        resolve_color(&track_visual.border_color, theme)
                            .map(|color| apply_opacity(color, track_visual.opacity))
                            .unwrap_or_else(|| control_border(node, theme, state)),
                        track_color,
                        track_radii,
                        track_border_w.max(border_w),
                    );
                } else {
                    out.push(inst_radii(track_rect, track_color, track_radii));
                }
                let t = state.slider_t(&node.id);
                let fill_w = track_w * t;
                if fill_w > 0.5 {
                    let fill_color = resolve_color(&fill_visual.background, theme)
                        .or_else(|| resolve_color(&fill_visual.foreground, theme))
                        .map(|color| apply_opacity(color, fill_visual.opacity))
                        .or(fill_fallback.background)
                        .unwrap_or(thumb_color);
                    let fill_radii = visual_radii_with_fallback(
                        &fill_visual,
                        [fill_w.min(track_h) * 0.5; 4],
                        sf,
                    );
                    out.push(inst_radii(
                        [x + margin, track_y, fill_w, track_h],
                        fill_color,
                        fill_radii,
                    ));
                }
                let thumb_layout = node.style.parts.parts.get("thumb").map(|part| &part.layout);
                let thumb_w = thumb_layout
                    .and_then(|layout| layout.width)
                    .map(|width| width.max(1.0) * sf)
                    .unwrap_or(SLIDER_THUMB_WIDTH_LP * sf);
                let thumb_h = thumb_layout
                    .and_then(|layout| layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(thumb_w);
                let thumb_min = x + margin;
                let thumb_max = (x + w - margin - thumb_w).max(thumb_min);
                let thumb_x =
                    (x + margin + t * track_w - thumb_w * 0.5).clamp(thumb_min, thumb_max);
                let thumb_y = y + (h - thumb_h) * 0.5;
                let thumb_border_w = thumb_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .unwrap_or(border_w);
                emit_bordered_rect_radii(
                    out,
                    [thumb_x, thumb_y, thumb_w, thumb_h],
                    resolve_color(&thumb_visual.border_color, theme)
                        .map(|color| apply_opacity(color, thumb_visual.opacity))
                        .or(styled_border)
                        .or(thumb_fallback.border_color)
                        .unwrap_or_else(|| control_border(node, theme, state)),
                    if state.is_disabled(&node.id) {
                        theme.disabled
                    } else {
                        thumb_color
                    },
                    visual_radii_with_fallback(&thumb_visual, [thumb_w.min(thumb_h) * 0.5; 4], sf),
                    thumb_border_w,
                );
            }

            WidgetKind::RangeSlider => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let track_visual = part_visual_for(node, state, "track");
                let range_visual = part_visual_for(node, state, "range");
                let thumb_min_visual = part_visual_for(node, state, "thumb-min");
                let thumb_max_visual = part_visual_for(node, state, "thumb-max");
                let track_fallback = widget_part_paint_fallback(node, "track", theme, state);
                let range_fallback = widget_part_paint_fallback(node, "range", theme, state);
                let thumb_min_fallback =
                    widget_part_paint_fallback(node, "thumb-min", theme, state);
                let thumb_max_fallback =
                    widget_part_paint_fallback(node, "thumb-max", theme, state);
                let track_color = resolve_color(&track_visual.background, theme)
                    .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)))
                    .or_else(|| {
                        resolve_color(&visual.track_color, theme)
                            .map(|color| apply_opacity(color, visual.opacity))
                    })
                    .or(paint_fallback.track_color)
                    .or(track_fallback.background)
                    .unwrap_or(theme.border);
                let fallback_thumb_color = resolve_color(&visual.thumb_color, theme)
                    .map(|color| apply_opacity(color, visual.opacity))
                    .or(styled_accent)
                    .or(paint_fallback.thumb_color)
                    .or(thumb_min_fallback.background)
                    .unwrap_or(theme.accent);
                let track_h = node
                    .style
                    .parts
                    .parts
                    .get("track")
                    .and_then(|part| part.layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(SLIDER_TRACK_HEIGHT_LP * sf)
                    .max(border_w);
                let track_y = y + (h - track_h) * 0.5;
                let margin = SLIDER_TRACK_MARGIN_LP * sf;
                let track_w = (w - 2.0 * margin).max(0.0);
                let track_rect = [x + margin, track_y, track_w, track_h];
                let track_radii = visual_radii_with_fallback(&track_visual, [track_h * 0.5; 4], sf);
                let track_border_w = track_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .unwrap_or(0.0);
                if track_border_w > 0.0 || track_visual.border_color.is_some() {
                    emit_bordered_rect_radii(
                        out,
                        track_rect,
                        resolve_color(&track_visual.border_color, theme)
                            .map(|color| apply_opacity(color, track_visual.opacity))
                            .unwrap_or_else(|| control_border(node, theme, state)),
                        track_color,
                        track_radii,
                        track_border_w.max(border_w),
                    );
                } else {
                    out.push(inst_radii(track_rect, track_color, track_radii));
                }

                let (t_min, t_max) = state.range_slider_t(&node.id);
                let range_x = x + margin + track_w * t_min.min(t_max);
                let range_w = track_w * (t_max - t_min).abs();
                if range_w > 0.5 {
                    let range_color = resolve_color(&range_visual.background, theme)
                        .or_else(|| resolve_color(&range_visual.foreground, theme))
                        .map(|color| apply_opacity(color, range_visual.opacity))
                        .or(range_fallback.background)
                        .unwrap_or(fallback_thumb_color);
                    let range_radii = visual_radii_with_fallback(
                        &range_visual,
                        [range_w.min(track_h) * 0.5; 4],
                        sf,
                    );
                    out.push(inst_radii(
                        [range_x, track_y, range_w, track_h],
                        range_color,
                        range_radii,
                    ));
                }

                let min_thumb_layout = node
                    .style
                    .parts
                    .parts
                    .get("thumb-min")
                    .map(|part| &part.layout);
                let min_thumb_w = min_thumb_layout
                    .and_then(|layout| layout.width)
                    .map(|width| width.max(1.0) * sf)
                    .unwrap_or(SLIDER_THUMB_WIDTH_LP * sf);
                let min_thumb_h = min_thumb_layout
                    .and_then(|layout| layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(min_thumb_w);
                let thumb_min_x = x + margin;
                let min_thumb_max_x = (x + w - margin - min_thumb_w).max(thumb_min_x);
                let min_thumb_x = (x + margin + t_min * track_w - min_thumb_w * 0.5)
                    .clamp(thumb_min_x, min_thumb_max_x);
                let min_thumb_y = y + (h - min_thumb_h) * 0.5;
                let min_thumb_border_w = thumb_min_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .unwrap_or(border_w);
                let min_thumb_color = resolve_color(&thumb_min_visual.background, theme)
                    .or_else(|| resolve_color(&thumb_min_visual.foreground, theme))
                    .map(|color| apply_opacity(color, thumb_min_visual.opacity.or(visual.opacity)))
                    .unwrap_or(fallback_thumb_color);
                emit_bordered_rect_radii(
                    out,
                    [min_thumb_x, min_thumb_y, min_thumb_w, min_thumb_h],
                    resolve_color(&thumb_min_visual.border_color, theme)
                        .map(|color| apply_opacity(color, thumb_min_visual.opacity))
                        .or(styled_border)
                        .or(thumb_min_fallback.border_color)
                        .unwrap_or_else(|| control_border(node, theme, state)),
                    if state.is_disabled(&node.id) {
                        theme.disabled
                    } else {
                        min_thumb_color
                    },
                    visual_radii_with_fallback(
                        &thumb_min_visual,
                        [min_thumb_w.min(min_thumb_h) * 0.5; 4],
                        sf,
                    ),
                    min_thumb_border_w,
                );

                let max_thumb_layout = node
                    .style
                    .parts
                    .parts
                    .get("thumb-max")
                    .map(|part| &part.layout);
                let max_thumb_w = max_thumb_layout
                    .and_then(|layout| layout.width)
                    .map(|width| width.max(1.0) * sf)
                    .unwrap_or(SLIDER_THUMB_WIDTH_LP * sf);
                let max_thumb_h = max_thumb_layout
                    .and_then(|layout| layout.height)
                    .map(|height| height.max(1.0) * sf)
                    .unwrap_or(max_thumb_w);
                let max_thumb_min_x = x + margin;
                let max_thumb_max_x = (x + w - margin - max_thumb_w).max(max_thumb_min_x);
                let max_thumb_x = (x + margin + t_max * track_w - max_thumb_w * 0.5)
                    .clamp(max_thumb_min_x, max_thumb_max_x);
                let max_thumb_y = y + (h - max_thumb_h) * 0.5;
                let max_thumb_border_w = thumb_max_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .unwrap_or(border_w);
                let max_thumb_color = resolve_color(&thumb_max_visual.background, theme)
                    .or_else(|| resolve_color(&thumb_max_visual.foreground, theme))
                    .map(|color| apply_opacity(color, thumb_max_visual.opacity.or(visual.opacity)))
                    .or(thumb_max_fallback.background)
                    .unwrap_or(fallback_thumb_color);
                emit_bordered_rect_radii(
                    out,
                    [max_thumb_x, max_thumb_y, max_thumb_w, max_thumb_h],
                    resolve_color(&thumb_max_visual.border_color, theme)
                        .map(|color| apply_opacity(color, thumb_max_visual.opacity))
                        .or(styled_border)
                        .or(thumb_max_fallback.border_color)
                        .unwrap_or_else(|| control_border(node, theme, state)),
                    if state.is_disabled(&node.id) {
                        theme.disabled
                    } else {
                        max_thumb_color
                    },
                    visual_radii_with_fallback(
                        &thumb_max_visual,
                        [max_thumb_w.min(max_thumb_h) * 0.5; 4],
                        sf,
                    ),
                    max_thumb_border_w,
                );
            }

            WidgetKind::Histogram => emit_histogram(
                out,
                node,
                theme,
                sf,
                [x, y, w, h],
                styled_bg.or(paint_fallback.background),
                styled_border.or(paint_fallback.border_color),
                styled_accent,
                radii,
                border_w,
            ),

            WidgetKind::BarChart => emit_bar_chart(
                out,
                node,
                theme,
                sf,
                [x, y, w, h],
                styled_bg.or(paint_fallback.background),
                styled_border.or(paint_fallback.border_color),
                styled_accent,
                radii,
                border_w,
            ),

            WidgetKind::Heatmap => emit_heatmap(
                out,
                node,
                theme,
                sf,
                [x, y, w, h],
                styled_bg.or(paint_fallback.background),
                styled_border.or(paint_fallback.border_color),
                radii,
                border_w,
            ),

            WidgetKind::PieChart => emit_pie_chart(
                out,
                node,
                theme,
                sf,
                [x, y, w, h],
                styled_bg.or(paint_fallback.background),
                styled_border.or(paint_fallback.border_color),
                radii,
                border_w,
            ),

            WidgetKind::LinePlot => emit_line_plot(
                out,
                node,
                theme,
                sf,
                [x, y, w, h],
                styled_bg.or(paint_fallback.background),
                styled_border.or(paint_fallback.border_color),
                radii,
                border_w,
            ),

            WidgetKind::Scatter3D => {
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border
                        .or(paint_fallback.border_color)
                        .unwrap_or(theme.border),
                    styled_bg
                        .or(paint_fallback.background)
                        .unwrap_or([0.0, 0.0, 0.0, 0.0]),
                    radii,
                    border_w,
                );
            }

            WidgetKind::DataFrameTable => {
                let header_visual = part_visual_for(node, state, "header");
                let row_visual = part_visual_for(node, state, "row");
                let selected_row_visual = part_visual_for(node, state, "row-selected");
                let grid_visual = part_visual_for(node, state, "grid-line");
                let header_fallback = widget_part_paint_fallback(node, "header", theme, state);
                let selected_row_fallback =
                    widget_part_paint_fallback(node, "row-selected", theme, state);
                let grid_fallback = widget_part_paint_fallback(node, "grid-line", theme, state);
                let grid_color = resolve_color(&grid_visual.background, theme)
                    .or_else(|| resolve_color(&grid_visual.foreground, theme))
                    .or_else(|| resolve_color(&grid_visual.border_color, theme))
                    .or(styled_border)
                    .or(grid_fallback.background)
                    .unwrap_or(theme.border);
                let grid_color = apply_opacity(grid_color, grid_visual.opacity);
                let grid_w = grid_visual
                    .border_width
                    .or_else(|| {
                        node.style
                            .parts
                            .parts
                            .get("grid-line")
                            .and_then(|part| part.layout.width)
                    })
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .or(grid_fallback.border_width.map(|width| width * sf))
                    .unwrap_or(border_w)
                    .max(1.0);
                let table_rect = [x, y, w, h];
                let table_radii = radii;
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                out.push(inst_radii(
                    table_rect,
                    styled_bg
                        .or(paint_fallback.background)
                        .unwrap_or(theme.surface),
                    radii,
                ));
                if let Some(table_state) = state.table(&node.id) {
                    let metrics = table::metrics_for_node(node, theme, sf);
                    let visible = table::visible(table_state, &full_rect, metrics);
                    let table_right = x + w;
                    let table_bottom = y + h;
                    let header_h = metrics.header_h.min(h);
                    let header_fill = resolve_color(&header_visual.background, theme)
                        .or_else(|| resolve_color(&header_visual.foreground, theme))
                        .map(|color| apply_opacity(color, header_visual.opacity))
                        .or(header_fallback.background)
                        .unwrap_or_else(|| mix(theme.surface_alt, theme.accent, 0.10));
                    push_masked_rect(
                        out,
                        table_rect,
                        header_fill,
                        table_radii,
                        [x, y, w, header_h],
                    );
                    if header_h < h {
                        push_masked_rect(
                            out,
                            table_rect,
                            grid_color,
                            table_radii,
                            [x, y + header_h, w, grid_w],
                        );
                    }
                    let index_line_x = x + metrics.index_w;
                    if index_line_x < table_right {
                        push_masked_rect(
                            out,
                            table_rect,
                            grid_color,
                            table_radii,
                            [index_line_x, y, grid_w, h],
                        );
                    }

                    for col_offset in 0..visible.col_count {
                        let Some((col_x, _)) =
                            table::column_bounds(table_state, &full_rect, metrics, col_offset)
                        else {
                            continue;
                        };
                        if col_x < table_right {
                            push_masked_rect(
                                out,
                                table_rect,
                                grid_color,
                                table_radii,
                                [col_x, y, grid_w, h],
                            );
                        }
                    }

                    if let Some((sort_target, direction)) = table_state.sort {
                        let indicator_w = DROPDOWN_CHEVRON_WIDTH_LP * sf;
                        let inset = (theme.spacing * 0.5 * sf).max(2.0 * sf);
                        let color =
                            single_part_mark_color(node, state, theme, "header", theme.muted_text);
                        let clip = Rect {
                            x,
                            y,
                            w,
                            h: header_h,
                        };
                        match sort_target {
                            TableSortColumn::Index if header_h > 0.0 => {
                                let marker_right = (x + metrics.index_w).min(table_right) - inset;
                                let marker_x = marker_right - indicator_w;
                                if marker_x > x && marker_right > marker_x {
                                    emit_triangle_chevron(
                                        out,
                                        [marker_x, y, indicator_w, header_h],
                                        color,
                                        matches!(direction, SortDirection::Asc),
                                        sf,
                                        Some(clip),
                                    );
                                }
                            }
                            TableSortColumn::Data(sort_col)
                                if sort_col >= visible.first_col
                                    && sort_col < visible.first_col + visible.col_count
                                    && header_h > 0.0 =>
                            {
                                if let Some((_, col_right)) = table::column_bounds(
                                    table_state,
                                    &full_rect,
                                    metrics,
                                    sort_col - visible.first_col,
                                ) {
                                    let marker_right = col_right.min(table_right) - inset;
                                    let marker_x = marker_right - indicator_w;
                                    if marker_x > x + metrics.index_w && marker_right > marker_x {
                                        emit_triangle_chevron(
                                            out,
                                            [marker_x, y, indicator_w, header_h],
                                            color,
                                            matches!(direction, SortDirection::Asc),
                                            sf,
                                            Some(clip),
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    let row_fill = resolve_color(&row_visual.background, theme)
                        .or_else(|| resolve_color(&row_visual.foreground, theme))
                        .map(|color| apply_opacity(color, row_visual.opacity));
                    let selected_row_fill = resolve_color(&selected_row_visual.background, theme)
                        .or_else(|| resolve_color(&selected_row_visual.foreground, theme))
                        .map(|color| apply_opacity(color, selected_row_visual.opacity));

                    for row_offset in 0..visible.row_count {
                        let row = visible.first_row + row_offset;
                        let Some((row_y, row_bottom)) =
                            table::row_bounds(&full_rect, metrics, row_offset)
                        else {
                            continue;
                        };
                        let row_h = row_bottom - row_y;
                        if table_state
                            .selected
                            .is_some_and(|(selected_row, _)| selected_row == row)
                        {
                            push_masked_rect(
                                out,
                                table_rect,
                                selected_row_fill
                                    .or(selected_row_fallback.background)
                                    .unwrap_or_else(|| mix(theme.surface_alt, theme.accent, 0.22)),
                                table_radii,
                                [x, row_y, w, row_h],
                            );
                        } else if let Some(row_fill) = row_fill {
                            push_masked_rect(
                                out,
                                table_rect,
                                row_fill,
                                table_radii,
                                [x, row_y, w, row_h],
                            );
                        } else if row % 2 == 1 {
                            push_masked_rect(
                                out,
                                table_rect,
                                mix(theme.surface, theme.surface_alt, 0.36),
                                table_radii,
                                [x, row_y, w, row_h],
                            );
                        }
                        if row_bottom < table_bottom {
                            push_masked_rect(
                                out,
                                table_rect,
                                grid_color,
                                table_radii,
                                [x, row_bottom, w, grid_w],
                            );
                        }
                    }

                    if let Some((_, selected_col)) = table_state.selected {
                        if selected_col >= visible.first_col
                            && selected_col < visible.first_col + visible.col_count
                        {
                            if let Some((col_x, col_right)) = table::column_bounds(
                                table_state,
                                &full_rect,
                                metrics,
                                selected_col - visible.first_col,
                            ) {
                                push_masked_rect(
                                    out,
                                    table_rect,
                                    [theme.accent[0], theme.accent[1], theme.accent[2], 0.12],
                                    table_radii,
                                    [col_x, y, col_right - col_x, h],
                                );
                            }
                        }
                    }
                    emit_table_scrollbar(node, state, theme, sf, full_rect, out);
                }
                let border_color = styled_border
                    .or(paint_fallback.border_color)
                    .unwrap_or(grid_color);
                if border_w > 0.0 {
                    out.push(inst_outline_ring_clipped(
                        table_rect,
                        border_color,
                        table_radii,
                        border_w,
                        default_local_clip(table_rect),
                    ));
                }
            }

            WidgetKind::Window
            | WidgetKind::HLayout
            | WidgetKind::VLayout
            | WidgetKind::ScrollArea
            | WidgetKind::GridLayout
            | WidgetKind::FlowLayout
            | WidgetKind::TreeView
            | WidgetKind::Pages
            | WidgetKind::Page
            | WidgetKind::Spacer
            | WidgetKind::Label
            | WidgetKind::ContextMenu
            | WidgetKind::MenuItem
            | WidgetKind::Tooltip
            | WidgetKind::Toast
            | WidgetKind::Unknown => {}
        }
        if side_border_overrides {
            emit_asymmetric_css_border(
                out,
                [x, y, w, h],
                radii,
                &visual,
                paint_fallback.border_color.unwrap_or(theme.border),
                theme,
                sf,
            );
        } else if patterned_uniform_border {
            let width = visual
                .border_width
                .or(paint_fallback.border_width)
                .unwrap_or(BORDER_WIDTH_LP)
                .max(0.0)
                * sf;
            let color = apply_opacity(
                resolve_color(&visual.border_color, theme)
                    .or(paint_fallback.border_color)
                    .unwrap_or(theme.border),
                visual.opacity,
            );
            if width > 0.0 && color[3] > 0.001 {
                let rect = [x, y, w, h];
                if let Some(local_clip) = local_clip_for_rect(rect, paint_clip) {
                    out.push(inst_patterned_outline_ring_clipped(
                        rect,
                        color,
                        radii,
                        width,
                        local_clip,
                        uniform_border_style,
                    ));
                }
            }
        }
        if let Some(filter) = visual
            .backdrop_filter
            .filter(|_| widget_supports_backdrop_filter(node.kind))
        {
            emit_backdrop_filter_tint(out, [x, y, w, h], radii, filter);
        }
        if widget_supports_box_shadow(node.kind) || node.kind == WidgetKind::Modal {
            emit_inset_box_shadows(out, [x, y, w, h], radii, &visual, theme, sf);
        }
        if widget_supports_outline(node.kind) {
            emit_outline(
                out,
                [full_rect.x, full_rect.y, full_rect.w, full_rect.h],
                radii,
                &visual,
                theme,
                sf,
                paint_clip,
            );
        }
        apply_background_noise_to_instances(
            &mut out[own_primitive_start..],
            visual
                .background_noise
                .or_else(|| backdrop_filter_noise(&visual)),
        );
    }

    let node_is_active_tab = node.kind == WidgetKind::Tab && state.is_active_tab(&node.id);
    let mut tab_body_context_available =
        node_is_active_tab || (context.tab_body_start && transparent_tab_body_container(node.kind));
    visit_stacking_children(node, |child| {
        let child_starts_tab_body =
            if tab_body_context_available && layout.visible_rect(&child.id).is_some() {
                tab_body_context_available = false;
                true
            } else {
                false
            };
        emit_rects_inner(
            child,
            layout,
            theme,
            sf,
            state,
            caret_positions,
            skip_open_modals,
            RenderContext {
                tab_body_start: child_starts_tab_body,
                transformed_ancestor: context.transformed_ancestor || subtree_transform.is_some(),
            },
            out,
            base_leaf_ranges.as_deref_mut(),
            icon_geometry_cache,
        );
    });
    if is_scroll_container_node(node) {
        if let Some(r) = layout.rects.get(&node.id).copied() {
            emit_panel_scrollbar(node, layout, state, theme, sf, [r.x, r.y, r.w, r.h], out);
        }
    }
    if let Some((transform, origin)) = subtree_transform {
        apply_transform_to_instances(
            &mut out[subtree_primitive_start..],
            Some(transform),
            sf,
            origin,
        );
    }
    apply_paint_clip(&mut out[subtree_primitive_start..], subtree_paint_clip);
    if !context.transformed_ancestor && !context.tab_body_start {
        if let Some(ranges) = base_leaf_ranges {
            ranges.insert(node.id.clone(), subtree_primitive_start..out.len());
        }
    }
}

fn splitter_is_horizontal(node: &WidgetNode) -> bool {
    node.props.orientation.as_deref().unwrap_or("horizontal") != "vertical"
}

fn splitter_gutter_size_px(node: &WidgetNode, horizontal: bool, sf: f32) -> f32 {
    let styled_size = node.style.parts.parts.get("gutter").and_then(|part| {
        if horizontal {
            part.layout.width
        } else {
            part.layout.height
        }
    });
    styled_size
        .or(node.props.gutter_size)
        .unwrap_or(6.0)
        .max(1.0)
        * sf
}

fn emit_splitter_gutters(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    splitter_rect: [f32; 4],
    out: &mut Vec<RectInstance>,
) {
    let horizontal = splitter_is_horizontal(node);
    let gutter_visual = part_visual_for(node, state, "gutter");
    let gutter_fallback = widget_part_paint_fallback(node, "gutter", theme, state);
    let preferred_size = splitter_gutter_size_px(node, horizontal, sf);
    let fallback_color = resolve_color(&gutter_visual.background, theme)
        .or_else(|| resolve_color(&gutter_visual.foreground, theme))
        .or_else(|| resolve_color(&gutter_visual.border_color, theme))
        .map(|color| apply_opacity(color, gutter_visual.opacity))
        .or_else(|| {
            gutter_fallback
                .background
                .map(|color| apply_opacity(color, gutter_visual.opacity))
        })
        .unwrap_or_else(|| apply_opacity(theme.border, gutter_visual.opacity));
    let gutter_fill = resolve_part_background_paint(&gutter_visual, theme, fallback_color, sf);
    let gutter_border = resolve_color(&gutter_visual.border_color, theme)
        .map(|color| apply_opacity(color, gutter_visual.opacity))
        .unwrap_or(fallback_color);
    let gutter_border_w = gutter_visual
        .border_width
        .map(|width| (width.max(0.0) * sf).max(0.0))
        .unwrap_or(0.0);
    let gutter_width_override = node
        .style
        .parts
        .parts
        .get("gutter")
        .and_then(|part| part.layout.width)
        .map(|width| width.max(1.0) * sf);
    let gutter_height_override = node
        .style
        .parts
        .parts
        .get("gutter")
        .and_then(|part| part.layout.height)
        .map(|height| height.max(1.0) * sf);
    let panes: Vec<&WidgetNode> = node
        .children
        .iter()
        .filter(|child| child.kind == WidgetKind::Pane && layout.rects.contains_key(&child.id))
        .collect();

    for pair in panes.windows(2) {
        let Some(before) = layout.rects.get(&pair[0].id).copied() else {
            continue;
        };
        let Some(after) = layout.rects.get(&pair[1].id).copied() else {
            continue;
        };
        let mut rect = if horizontal {
            let before_end = before.x + before.w;
            let after_start = after.x;
            let gap = after_start - before_end;
            let gutter_w = if gap > 0.5 { gap } else { preferred_size };
            [
                if gap > 0.5 {
                    before_end
                } else {
                    before_end - gutter_w * 0.5
                },
                splitter_rect[1],
                gutter_w,
                splitter_rect[3],
            ]
        } else {
            let before_end = before.y + before.h;
            let after_start = after.y;
            let gap = after_start - before_end;
            let gutter_h = if gap > 0.5 { gap } else { preferred_size };
            [
                splitter_rect[0],
                if gap > 0.5 {
                    before_end
                } else {
                    before_end - gutter_h * 0.5
                },
                splitter_rect[2],
                gutter_h,
            ]
        };

        if horizontal {
            let width = gutter_width_override
                .unwrap_or(2.0 * sf)
                .min(rect[2])
                .max(1.0);
            rect[0] += (rect[2] - width) * 0.5;
            rect[2] = width;
        } else {
            let height = gutter_height_override
                .unwrap_or(2.0 * sf)
                .min(rect[3])
                .max(1.0);
            rect[1] += (rect[3] - height) * 0.5;
            rect[3] = height;
        }

        if rect[2] <= 0.0 || rect[3] <= 0.0 {
            continue;
        }
        let radius = rect[2].min(rect[3]) * 0.5;
        let gutter_radii = visual_radii_with_fallback(&gutter_visual, [radius; 4], sf);
        if gutter_border_w > 0.0 {
            emit_bordered_paint_rect_radii(
                out,
                rect,
                gutter_border,
                gutter_fill.clone(),
                gutter_radii,
                gutter_border_w,
            );
        } else {
            emit_paint_rect_radii(out, rect, gutter_fill.clone(), gutter_radii);
        }
    }
}

fn led_state_is_off(node: &WidgetNode) -> bool {
    matches!(
        node.props
            .led_state
            .as_deref()
            .unwrap_or("off")
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "off" | "false" | "0" | "inactive" | "disabled"
    )
}

fn led_default_color(node: &WidgetNode, theme: &Theme) -> [f32; 4] {
    if led_state_is_off(node) {
        theme.disabled
    } else {
        theme.success
    }
}

fn caret_xy_for_node(
    left: f32,
    text_width: f32,
    id: &str,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
) -> [f32; 2] {
    let fallback = [text_width * state.caret_t(id), 0.0];
    let xy = caret_positions.get(id).copied().unwrap_or(fallback);
    [left + xy[0].clamp(0.0, text_width), xy[1]]
}

fn emit_text_selection_rects(
    out: &mut Vec<RectInstance>,
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    rect: [f32; 4],
    border_w: f32,
    gutter_w: f32,
) {
    let Some((start, end)) = state.normalized_text_selection(&node.id) else {
        return;
    };
    let Some(value) = state.text_val.get(&node.id) else {
        return;
    };
    if start >= end || value.is_empty() {
        return;
    }
    let pad = theme.spacing * sf;
    let mut color = theme.focus;
    color[3] = 0.34;
    let [x, y, w, h] = rect;
    let mut text_left = x + pad + gutter_w;
    let mut text_top = y + pad;
    let mut text_w = (w - pad * 2.0 - gutter_w).max(1.0);
    let mut scroll_x = 0.0;
    let mut scroll_y = 0.0;
    let mut wrap = false;
    let multiline = matches!(
        node.kind,
        WidgetKind::TextArea | WidgetKind::CodeEditor | WidgetKind::LogView
    );
    if node.kind == WidgetKind::NumberInput {
        let step_w = number_stepper_width_for_style(&node.style, w, sf);
        let font_size = crate::text::text_font_size(node, theme, sf);
        let line_h = crate::text::text_line_height(font_size, theme, sf).max(1.0);
        text_left = x + step_w + pad;
        text_top = y + (h - line_h) * 0.5;
        text_w = (w - step_w * 2.0 - pad * 2.0).max(1.0);
    } else if !multiline {
        let font_size = crate::text::text_font_size(node, theme, sf);
        let line_h = crate::text::text_line_height(font_size, theme, sf).max(1.0);
        text_top = y + (h - line_h) * 0.5;
    } else {
        wrap = node.props.wrap.unwrap_or(true);
        let font_size = crate::text::text_font_size(node, theme, sf);
        let line_h = crate::text::text_line_height(font_size, theme, sf).max(1.0);
        let visible_h = (h - pad * 2.0).max(1.0);
        scroll_x = if wrap {
            0.0
        } else {
            state.text_area_scroll_x(&node.id)
        };
        scroll_y = state.text_area_scroll_y(&node.id, visible_h, line_h);
    }

    let clip = [
        x + border_w,
        y + border_w,
        (w - border_w * 2.0).max(0.0),
        (h - border_w * 2.0).max(0.0),
    ];
    for [local_x, local_y, local_w, local_h] in
        crate::text::shaped_text_selection_rects(node, theme, sf, value, text_w, wrap, (start, end))
    {
        let row = [
            text_left + local_x - scroll_x,
            text_top + local_y - scroll_y,
            local_w,
            local_h,
        ];
        if let Some(r) = intersect_rect_arrays(row, clip) {
            out.push(inst(r, color, 2.0 * sf));
        }
    }
}

fn intersect_rect_arrays(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

fn badge_rect(
    node: &WidgetNode,
    rect: [f32; 4],
    theme: &Theme,
    sf: f32,
    right_inset: f32,
) -> Option<[f32; 4]> {
    let badge = node
        .props
        .badge
        .as_deref()
        .filter(|badge| !badge.is_empty())?;
    let layout =
        inline_badge_layout_for_text(&node.style, badge, theme, sf, rect[2], rect[3], right_inset);
    layout
        .visible_rect
        .map(|[x, y, w, h]| [rect[0] + x, rect[1] + y, w, h])
}

fn emit_badge_pill(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    rect: [f32; 4],
    out: &mut Vec<RectInstance>,
) {
    let visual = part_visual_for(node, state, "badge");
    let fill = apply_opacity(
        resolve_color(&visual.background, theme)
            .or(resolve_color(&visual.foreground, theme))
            .unwrap_or(theme.accent),
        visual.opacity,
    );
    let border_w = visual
        .border_width
        .map(|width| (width.max(0.0) * sf).max(0.0))
        .unwrap_or(0.0);
    let radii = visual_radii_with_fallback(&visual, [rect[3] * 0.5; 4], sf);
    if border_w > 0.0 {
        emit_bordered_rect_radii(
            out,
            rect,
            resolve_color(&visual.border_color, theme).unwrap_or(fill),
            fill,
            radii,
            border_w,
        );
    } else {
        out.push(inst_radii(rect, fill, radii));
    }
}

fn emit_menu_overlays(
    tree: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    out: &mut Vec<RectInstance>,
) {
    if let Some(menu_id) = state.open_menu.as_deref() {
        if let Some(rect) = menu_popup_rect(tree, layout, state, theme, sf, menu_id) {
            if let Some(items) = state.menu_items.get(menu_id) {
                emit_menu_popup(tree, rect, items, theme, sf, state, menu_id, out);
            }
        }
    }
    if let Some(menu_id) = state.open_context_menu.as_deref() {
        if let Some(rect) = menu_popup_rect(tree, layout, state, theme, sf, menu_id) {
            if let Some(items) = state.menu_items.get(menu_id) {
                emit_menu_popup(tree, rect, items, theme, sf, state, menu_id, out);
            }
        }
    }
}

fn emit_modal_overlays(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
    out: &mut Vec<RectInstance>,
    icon_geometry_cache: &mut IconGeometryCache,
) {
    if node.kind == WidgetKind::Modal && node.props.open.unwrap_or(false) {
        emit_rects_inner(
            node,
            layout,
            theme,
            sf,
            state,
            caret_positions,
            false,
            RenderContext::default(),
            out,
            None,
            icon_geometry_cache,
        );
        return;
    }
    for child in &node.children {
        emit_modal_overlays(
            child,
            layout,
            theme,
            sf,
            state,
            caret_positions,
            out,
            icon_geometry_cache,
        );
    }
}

fn emit_menu_popup(
    tree: &WidgetNode,
    rect: Rect,
    items: &[NavigationItem],
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    menu_id: &str,
    out: &mut Vec<RectInstance>,
) {
    if items.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let menu = find_node(tree, menu_id);
    let menu_kind = menu.map(|node| node.kind).unwrap_or(WidgetKind::Menu);
    let menu_visual = menu
        .map(|node| part_visual_for(node, state, "menu"))
        .unwrap_or_default();
    let menu_fallback = native_widget_part_paint_fallback(
        menu_kind,
        "menu",
        theme,
        PaintInteraction::Resting,
        false,
    );
    let radius_lp = menu_visual
        .border_radius
        .or(menu_fallback.border_radius)
        .unwrap_or(theme.radius)
        .max(0.0);
    let radius = radius_lp * sf;
    let radii = visual_radii(&menu_visual, radius_lp, sf);
    let border_w = menu_visual
        .border_width
        .map(|width| width.max(0.0) * sf)
        .or(menu_fallback.border_width.map(|width| width * sf))
        .unwrap_or(BORDER_WIDTH_LP * sf);
    let row_h = theme.control_height() * sf;
    let popup_rect = [rect.x, rect.y, rect.w, rect.h];
    if menu_visual.box_shadows.is_some() {
        emit_box_shadows(out, popup_rect, radii, &menu_visual, theme, sf, None);
    } else {
        let shadow_offset = 3.0 * sf;
        out.push(inst(
            [
                rect.x + shadow_offset,
                rect.y + shadow_offset,
                rect.w,
                rect.h,
            ],
            [0.0, 0.0, 0.0, 0.30],
            radius,
        ));
    }
    let border_color = resolve_color(&menu_visual.border_color, theme)
        .map(|color| apply_opacity(color, menu_visual.opacity))
        .or(menu_fallback.border_color)
        .unwrap_or_else(|| mix(theme.border, theme.accent, 0.18));
    emit_paint_rect_radii(
        out,
        popup_rect,
        resolve_part_background_paint(
            &menu_visual,
            theme,
            menu_fallback.background.unwrap_or(theme.surface),
            sf,
        ),
        radii,
    );
    for (idx, item) in items.iter().enumerate() {
        let y = rect.y + idx as f32 * row_h;
        if y >= rect.y + rect.h {
            break;
        }
        let disabled = item.disabled || state.is_disabled(&item.id);
        let hovered = state.hovered.as_deref() == Some(item.id.as_str());
        let row_visual = menu
            .map(|node| {
                if disabled {
                    merged_part_visual_for(node, state, &["item", "item-disabled"])
                } else if hovered {
                    merged_part_visual_for(node, state, &["item", "item-hover"])
                } else {
                    part_visual_for(node, state, "item")
                }
            })
            .unwrap_or_default();
        let row_part = if disabled {
            "item-disabled"
        } else if hovered {
            "item-hover"
        } else {
            "item"
        };
        let row_fallback = native_widget_part_paint_fallback(
            menu_kind,
            row_part,
            theme,
            PaintInteraction::Resting,
            false,
        );
        let color = if let Some(color) = resolve_color(&row_visual.background, theme)
            .or_else(|| resolve_color(&row_visual.foreground, theme))
        {
            apply_opacity(color, row_visual.opacity)
        } else {
            row_fallback.background.unwrap_or_else(|| {
                if disabled {
                    mix(theme.surface, theme.disabled, 0.18)
                } else if hovered {
                    mix(theme.surface_alt, theme.accent, 0.24)
                } else {
                    theme.surface_alt
                }
            })
        };
        push_masked_rect(
            out,
            popup_rect,
            color,
            radii,
            [
                rect.x + border_w,
                y + border_w,
                rect.w - border_w * 2.0,
                row_h.min(rect.y + rect.h - y) - border_w,
            ],
        );
    }
    if border_w > 0.0 {
        out.push(inst_outline_ring_clipped(
            popup_rect,
            border_color,
            radii,
            border_w,
            default_local_clip(popup_rect),
        ));
    }
}

fn root_rect(layout: &LayoutResult) -> Option<Rect> {
    let mut iter = layout.rects.values().copied();
    let first = iter.next()?;
    let (mut left, mut top, mut right, mut bottom) =
        (first.x, first.y, first.x + first.w, first.y + first.h);
    for r in iter {
        left = left.min(r.x);
        top = top.min(r.y);
        right = right.max(r.x + r.w);
        bottom = bottom.max(r.y + r.h);
    }
    Some(Rect {
        x: left,
        y: top,
        w: (right - left).max(0.0),
        h: (bottom - top).max(0.0),
    })
}

fn emit_dropdown_overlays(
    tree: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    out: &mut Vec<RectInstance>,
) {
    let overlay = dropdown_overlay_rect(tree, layout, state, theme, sf);
    emit_dropdown_overlay_nodes(tree, layout, theme, sf, state, overlay, out);
}

fn emit_dropdown_overlay_nodes(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    overlay: Option<Rect>,
    out: &mut Vec<RectInstance>,
) {
    if node.kind == WidgetKind::Dropdown && state.open_dropdown.as_deref() == Some(node.id.as_str())
    {
        if let (Some(menu), Some(items)) = (overlay, state.dropdown_items.get(&node.id)) {
            let row_h = theme.control_height() * sf;
            let menu_visual = part_visual_for(node, state, "menu");
            let menu_fallback = native_widget_part_paint_fallback(
                WidgetKind::Dropdown,
                "menu",
                theme,
                PaintInteraction::Resting,
                false,
            );
            let menu_radius_lp = menu_visual
                .border_radius
                .or(menu_fallback.border_radius)
                .unwrap_or(theme.radius)
                .max(0.0);
            let menu_radii = visual_radii(&menu_visual, menu_radius_lp, sf);
            let border_w = menu_visual
                .border_width
                .or(menu_fallback.border_width)
                .unwrap_or(BORDER_WIDTH_LP)
                .max(0.0)
                * sf;
            let menu_rect = [menu.x, menu.y, menu.w, menu.h];
            if menu_visual.box_shadows.is_some() {
                emit_box_shadows(out, menu_rect, menu_radii, &menu_visual, theme, sf, None);
            } else {
                let shadow_offset = 3.0 * sf;
                out.push(inst_radii(
                    [
                        menu_rect[0] + shadow_offset,
                        menu_rect[1] + shadow_offset,
                        menu_rect[2],
                        menu_rect[3],
                    ],
                    [0.0, 0.0, 0.0, 0.30],
                    menu_radii,
                ));
            }
            let border_color = resolve_color(&menu_visual.border_color, theme)
                .map(|color| apply_opacity(color, menu_visual.opacity))
                .or(menu_fallback.border_color)
                .unwrap_or_else(|| mix(theme.border, theme.accent, 0.18));
            let fill_color = resolve_color(&menu_visual.background, theme)
                .map(|color| apply_opacity(color, menu_visual.opacity))
                .or(menu_fallback.background)
                .unwrap_or(theme.surface);
            out.push(inst_radii(menu_rect, fill_color, menu_radii));
            let selected = state.dropdown_index.get(&node.id).copied().unwrap_or(0);
            let hovered = state
                .dropdown_hover
                .as_ref()
                .filter(|(id, _)| id == &node.id)
                .map(|(_, idx)| *idx);
            for idx in 0..items.len() {
                let y = menu.y + idx as f32 * row_h;
                if y >= menu.y + menu.h {
                    break;
                }
                let row_visual = if Some(idx) == hovered && idx == selected {
                    merged_part_visual_for(node, state, &["item", "item-selected", "item-hover"])
                } else if Some(idx) == hovered {
                    merged_part_visual_for(node, state, &["item", "item-hover"])
                } else if idx == selected {
                    merged_part_visual_for(node, state, &["item", "item-selected"])
                } else {
                    part_visual_for(node, state, "item")
                };
                let row_fallback = native_widget_part_paint_fallback_with_selection(
                    WidgetKind::Dropdown,
                    "item",
                    theme,
                    if Some(idx) == hovered {
                        PaintInteraction::Hovered
                    } else {
                        PaintInteraction::Resting
                    },
                    false,
                    idx == selected,
                );
                let color = resolve_color(&row_visual.background, theme)
                    .map(|color| apply_opacity(color, row_visual.opacity))
                    .or(row_fallback.background)
                    .unwrap_or(theme.surface_alt);
                push_masked_rect(
                    out,
                    menu_rect,
                    color,
                    menu_radii,
                    [
                        menu.x + border_w,
                        y + border_w,
                        menu.w - border_w * 2.0,
                        row_h.min(menu.y + menu.h - y) - border_w,
                    ],
                );
            }
            if border_w > 0.0 {
                out.push(inst_outline_ring_clipped(
                    menu_rect,
                    border_color,
                    menu_radii,
                    border_w,
                    default_local_clip(menu_rect),
                ));
            }
        }
    }

    for child in &node.children {
        emit_dropdown_overlay_nodes(child, layout, theme, sf, state, overlay, out);
    }
}

fn emit_tooltip_overlay(
    tree: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
    stylesheets: &StylesheetStore,
    media: DgMediaEnvironment,
    out: &mut Vec<RectInstance>,
) {
    if let Some((node, rect)) = rich_tooltip_target(tree, layout, state) {
        emit_tooltip_surface(node, rect, theme, sf, state, out);
        for child in &node.children {
            emit_rects(child, layout, theme, sf, state, caret_positions, out);
        }
        return;
    }
    let Some((_node, rect)) = tooltip_target(tree, layout, theme, state, sf, stylesheets, media)
    else {
        return;
    };
    let style = computed_style_for_virtual_element_with_media(
        WidgetKind::Tooltip,
        "__dg_static_tooltip",
        &["static"],
        stylesheets,
        Some(media),
    );
    emit_static_tooltip_surface(rect, theme, sf, &style, out);
}

fn emit_tooltip_surface(
    node: &WidgetNode,
    rect: Rect,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    out: &mut Vec<RectInstance>,
) {
    let paint_fallback = widget_paint_fallback(node, theme, state);
    let border_w = paint_fallback.border_width.unwrap_or(BORDER_WIDTH_LP) * sf;
    let visual = visual_for(node, state, theme);
    let radius_lp = visual
        .border_radius
        .or(paint_fallback.border_radius)
        .unwrap_or(theme.radius)
        .max(0.0);
    let radius = radius_lp * sf;
    if visual.box_shadows.is_some() {
        emit_box_shadows(
            out,
            [rect.x, rect.y, rect.w, rect.h],
            [radius; 4],
            &visual,
            theme,
            sf,
            None,
        );
    } else {
        let shadow = 4.0 * sf;
        out.push(inst(
            [rect.x + shadow, rect.y + shadow, rect.w, rect.h],
            [0.0, 0.0, 0.0, 0.36],
            radius,
        ));
    }
    emit_bordered_rect(
        out,
        [rect.x, rect.y, rect.w, rect.h],
        resolve_color(&visual.border_color, theme)
            .map(|color| apply_opacity(color, visual.opacity))
            .or(paint_fallback.border_color)
            .unwrap_or_else(|| mix(theme.border, theme.accent, 0.18)),
        resolve_color(&visual.background, theme)
            .map(|color| apply_opacity(color, visual.opacity))
            .or(paint_fallback.background)
            .unwrap_or(theme.surface_alt),
        radius,
        visual
            .border_width
            .map(|width| width.max(0.0) * sf)
            .unwrap_or(border_w),
    );
}

fn emit_static_tooltip_surface(
    rect: Rect,
    theme: &Theme,
    sf: f32,
    style: &NodeStyle,
    out: &mut Vec<RectInstance>,
) {
    let paint_fallback = native_widget_paint_fallback_with_level(
        WidgetKind::Tooltip,
        None,
        theme,
        PaintInteraction::Resting,
    );
    let border_w = paint_fallback.border_width.unwrap_or(BORDER_WIDTH_LP) * sf;
    let opacity = resolve_overlay_opacity(style, 1.0);
    let radius = overlay_radius(style, theme.radius, sf);
    if style.visual.box_shadows.is_some() {
        let mut shadow_visual = style.visual.clone();
        shadow_visual.opacity = Some(opacity);
        emit_box_shadows(
            out,
            [rect.x, rect.y, rect.w, rect.h],
            [radius; 4],
            &shadow_visual,
            theme,
            sf,
            None,
        );
    } else {
        let shadow = 4.0 * sf;
        out.push(inst(
            [rect.x + shadow, rect.y + shadow, rect.w, rect.h],
            [0.0, 0.0, 0.0, 0.36 * opacity],
            radius,
        ));
    }
    emit_bordered_rect(
        out,
        [rect.x, rect.y, rect.w, rect.h],
        overlay_color(
            &style.visual.border_color,
            theme,
            paint_fallback
                .border_color
                .unwrap_or_else(|| mix(theme.border, theme.accent, 0.18)),
            opacity,
        ),
        overlay_color(
            &style.visual.background,
            theme,
            with_alpha(paint_fallback.background.unwrap_or(theme.surface_alt), 1.0),
            opacity,
        ),
        radius,
        style
            .visual
            .border_width
            .map(|width| width.max(0.0) * sf)
            .unwrap_or(border_w),
    );
}

fn emit_toast_overlays(
    toasts: &[ToastOverlay],
    theme: &Theme,
    sf: f32,
    stylesheets: &StylesheetStore,
    media: DgMediaEnvironment,
    window_w: f32,
    window_h: f32,
    out: &mut Vec<RectInstance>,
) {
    let border_w = BORDER_WIDTH_LP * sf;
    let mut stack_counts = [0usize; 4];
    for toast in toasts {
        let classes = [toast.level.as_str()];
        let style = computed_style_for_virtual_element_with_media(
            WidgetKind::Toast,
            toast.id.as_str(),
            &classes,
            stylesheets,
            Some(media),
        );
        let idx = toast_stack_index(toast.position, &mut stack_counts);
        let padding = toast
            .padding
            .or_else(|| uniform_layout_padding(&style.layout));
        let rect = toast_rect(
            idx,
            &toast.message,
            window_w,
            window_h,
            sf,
            toast.position,
            padding,
            &style.text,
            theme,
        );
        if rect.w <= 0.0 || rect.h <= 0.0 {
            continue;
        }
        let radius = toast
            .radius
            .map(|radius| radius.max(0.0) * sf)
            .unwrap_or_else(|| overlay_radius(&style, theme.radius, sf));
        let colors = toast_colors(toast.level, theme, 1.0);
        let opacity = resolve_overlay_opacity(&style, toast.opacity);
        let fill = overlay_color(&style.visual.background, theme, colors.fill, opacity);
        let border = overlay_color(&style.visual.border_color, theme, colors.border, opacity);
        if style.visual.box_shadows.is_some() {
            let mut shadow_visual = style.visual.clone();
            shadow_visual.opacity = Some(opacity);
            emit_box_shadows(
                out,
                [rect.x, rect.y, rect.w, rect.h],
                [radius; 4],
                &shadow_visual,
                theme,
                sf,
                None,
            );
        } else {
            let shadow = 4.0 * sf;
            out.push(inst(
                [rect.x + shadow, rect.y + shadow, rect.w, rect.h],
                [0.0, 0.0, 0.0, 0.36 * opacity],
                radius,
            ));
        }
        emit_bordered_rect(
            out,
            [rect.x, rect.y, rect.w, rect.h],
            border,
            fill,
            radius,
            style
                .visual
                .border_width
                .map(|width| width.max(0.0) * sf)
                .unwrap_or(border_w),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        BarChartSeriesProp, HeatmapHoverProp, LinePlotPayloadFormat, LinePlotSeriesProp, NodeProps,
    };
    use crate::style::{
        BackdropFilterStyle, BackgroundPaint, BlobGradient, BlobGradientStop, BoxShadow,
        CalcLength, ColorRef, GradientStop, LinearGradient, MeshGradient, OverflowStyle,
        PartLayoutStyle, PartStyle, RadialGradient, TextStyle, VisualStyle,
    };

    fn node(id: &str, kind: WidgetKind) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name: None,
            css_types: Vec::new(),
            kind,
            props: NodeProps::default(),
            style_json: Default::default(),
            default_style: Default::default(),
            inline_style: Default::default(),
            style: Default::default(),
            children: Vec::new(),
        }
    }

    fn icon_resource(stroke_width: f64) -> Value {
        serde_json::json!({
            "view_box": [0.0, 0.0, 24.0, 24.0],
            "stroke_width": stroke_width,
            "strokes": [
                {
                    "points": [[3.0, 12.0], [12.0, 3.0], [21.0, 12.0]],
                    "closed": true
                }
            ]
        })
    }

    #[test]
    fn parsed_icon_geometry_emits_without_transformed_point_allocation_contract_changes() {
        let geometry =
            parse_custom_icon_resource(&icon_resource(2.0)).expect("valid icon resource");
        assert_eq!(geometry.view_box, [0.0, 0.0, 24.0, 24.0]);
        assert_eq!(geometry.strokes.len(), 1);
        assert_eq!(geometry.strokes[0].points.len(), 3);

        let mut out = Vec::new();
        emit_custom_icon_geometry(&mut out, &geometry, [10.0, 20.0, 40.0, 40.0], [1.0; 4]);
        assert_eq!(out.len(), 3, "closed triangle should emit three segments");
    }

    #[test]
    fn icon_geometry_cache_reuses_steady_state_resource() {
        let resource = icon_resource(2.0);
        let mut cache = IconGeometryCache::with_capacity(4);
        let first = cache.resolve(&resource).expect("first parse");
        for _ in 0..999 {
            let cached = cache.resolve(&resource).expect("cached geometry");
            assert!(Arc::ptr_eq(&first, &cached));
        }
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.misses, 1);
        assert_eq!(cache.hits, 999);
        assert_eq!(cache.evictions, 0);
        assert_eq!(cache.parse_failures, 0);
    }

    #[test]
    fn icon_geometry_cache_bounds_theme_replacement_history() {
        let mut cache = IconGeometryCache::with_capacity(2);
        cache.resolve(&icon_resource(1.0)).expect("first resource");
        cache.resolve(&icon_resource(2.0)).expect("second resource");
        cache.resolve(&icon_resource(3.0)).expect("third resource");

        assert_eq!(cache.entries.len(), 2);
        assert_eq!(cache.misses, 3);
        assert_eq!(cache.evictions, 1);
        cache
            .resolve(&icon_resource(1.0))
            .expect("evicted resource reparses");
        assert_eq!(cache.misses, 4);
        assert_eq!(cache.evictions, 2);
    }

    #[test]
    fn icon_geometry_cache_reports_invalid_resources_without_retaining_them() {
        let invalid = serde_json::json!({
            "view_box": [0.0, 0.0, 0.0, 24.0],
            "stroke_width": 2.0,
            "strokes": [{"points": [[0.0, 0.0], [1.0, 1.0]]}]
        });
        let mut cache = IconGeometryCache::with_capacity(4);
        assert!(cache.resolve(&invalid).is_none());
        assert!(cache.resolve(&invalid).is_none());
        assert!(cache.entries.is_empty());
        assert_eq!(cache.misses, 2);
        assert_eq!(cache.parse_failures, 2);
    }

    #[test]
    fn primitive_renderer_stats_expose_icon_cache_diagnostics() {
        let stats = PrimitiveRendererStats {
            icon_cache_capacity: 128,
            icon_cache_entries: 3,
            icon_cache_hits: 40,
            icon_cache_misses: 4,
            icon_cache_evictions: 1,
            icon_cache_parse_failures: 2,
            ..PrimitiveRendererStats::default()
        };
        assert_eq!(
            stats.icon_geometry_cache_snapshot(),
            serde_json::json!({
                "capacity": 128,
                "entries": 3,
                "hits": 40,
                "misses": 4,
                "evictions": 1,
                "parse_failures": 2,
            })
        );
    }

    #[test]
    fn complex_rect_shader_with_border_patterns_parses_and_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("rect.wgsl"))
            .expect("rect shader should parse");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("rect shader should validate");
    }

    #[test]
    fn background_patterns_use_one_dpi_scaled_rounded_rect_instance() {
        let visual = VisualStyle {
            opacity: Some(0.5),
            background_paint: Some(BackgroundPaint::Pattern(crate::style::BackgroundPattern {
                kind: BackgroundPatternKind::DiagonalHatch,
                foreground: ColorRef::Rgba([1.0, 0.5, 0.25, 0.8]),
                background: ColorRef::Rgba([0.1, 0.2, 0.3, 0.6]),
                tile_size: 8.0,
            })),
            ..VisualStyle::default()
        };
        let paint = resolve_background_paint(&visual, &Theme::dark(), [0.0; 4], 2.0);
        let FillPaint::Pattern {
            kind,
            foreground,
            background,
            tile_size_px,
        } = paint
        else {
            panic!("resolved background pattern");
        };
        assert_eq!(kind, BackgroundPatternKind::DiagonalHatch);
        assert_eq!(tile_size_px, 16.0);
        assert_eq!(foreground[3], 0.4);
        assert_eq!(background[3], 0.3);

        let mut out = Vec::new();
        emit_paint_rect_radii(
            &mut out,
            [10.0, 20.0, 180.0, 80.0],
            FillPaint::Pattern {
                kind,
                foreground,
                background,
                tile_size_px,
            },
            [9.0; 4],
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].paint, [5.0, 3.0, 16.0, 0.0]);
        assert_eq!(out[0].radii, [9.0; 4]);
        assert_eq!(out[0].color, foreground);
        assert_eq!(out[0].color2, background);
    }

    #[test]
    fn asymmetric_solid_border_emits_one_clamped_strip_per_visible_edge() {
        let visual = VisualStyle {
            border_top_width: Some(1.0),
            border_right_width: Some(2.0),
            border_bottom_width: Some(3.0),
            border_left_width: Some(4.0),
            border_top_color: Some(ColorRef::Rgba([1.0, 0.0, 0.0, 1.0])),
            border_right_color: Some(ColorRef::Rgba([0.0, 1.0, 0.0, 1.0])),
            border_bottom_color: Some(ColorRef::Rgba([0.0, 0.0, 1.0, 1.0])),
            border_left_color: Some(ColorRef::Rgba([1.0, 1.0, 0.0, 1.0])),
            ..VisualStyle::default()
        };
        let mut out = Vec::new();

        emit_asymmetric_css_border(
            &mut out,
            [10.0, 20.0, 100.0, 50.0],
            [5.0; 4],
            &visual,
            [0.5; 4],
            &Theme::dark(),
            1.0,
        );

        assert_eq!(out.len(), 4);
        assert_eq!(out[0].rect, [10.0, 20.0, 4.0, 50.0]);
        assert_eq!(out[1].rect, [108.0, 20.0, 2.0, 50.0]);
        assert_eq!(out[2].rect, [10.0, 20.0, 100.0, 1.0]);
        assert_eq!(out[3].rect, [10.0, 67.0, 100.0, 3.0]);
        assert_eq!(out[0].color, [1.0, 1.0, 0.0, 1.0]);
        assert_eq!(out[2].color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn side_color_override_repaints_uniform_width_without_geometry_override() {
        let visual = VisualStyle {
            border_width: Some(2.0),
            border_color: Some(ColorRef::Rgba([0.2, 0.2, 0.2, 1.0])),
            border_right_color: Some(ColorRef::Rgba([0.0, 1.0, 0.0, 1.0])),
            ..VisualStyle::default()
        };
        assert!(visual.has_border_side_overrides());
        let mut out = Vec::new();

        emit_asymmetric_css_border(
            &mut out,
            [0.0, 0.0, 20.0, 10.0],
            [0.0; 4],
            &visual,
            [0.5; 4],
            &Theme::dark(),
            1.0,
        );

        assert_eq!(out.len(), 4);
        assert_eq!(out[1].color, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(out[0].color, [0.2, 0.2, 0.2, 1.0]);
        assert_eq!(out[2].rect[3], 2.0);
    }

    #[test]
    fn patterned_ring_instances_encode_gpu_pattern_without_extra_geometry() {
        for (style, code) in [
            (BorderLineStyle::Dotted, 10.0),
            (BorderLineStyle::Dashed, 11.0),
            (BorderLineStyle::Double, 12.0),
        ] {
            let instance = inst_patterned_outline_ring_clipped(
                [2.0, 3.0, 80.0, 30.0],
                [1.0; 4],
                [8.0; 4],
                3.0,
                [0.0, 0.0, 80.0, 30.0],
                style,
            );
            assert_eq!(instance.params[2], 3.0);
            assert_eq!(instance.paint[0], code);
            assert_eq!(instance.paint[3], 3.0);
        }
    }

    #[test]
    fn side_patterns_use_one_dpi_scaled_gpu_strip_per_visible_edge() {
        let visual = VisualStyle {
            border_top_width: Some(2.0),
            border_right_width: Some(3.0),
            border_bottom_width: Some(4.0),
            border_left_width: Some(5.0),
            border_top_style: Some(BorderLineStyle::Dashed),
            border_right_style: Some(BorderLineStyle::Dotted),
            border_bottom_style: Some(BorderLineStyle::Double),
            border_left_style: Some(BorderLineStyle::None),
            ..VisualStyle::default()
        };
        let mut out = Vec::new();

        emit_asymmetric_css_border(
            &mut out,
            [0.0, 0.0, 100.0, 50.0],
            [6.0; 4],
            &visual,
            [1.0; 4],
            &Theme::dark(),
            1.25,
        );

        assert_eq!(out.len(), 3);
        assert_eq!(out[0].rect, [96.25, 0.0, 3.75, 50.0]);
        assert_eq!(out[0].paint[0], 10.0);
        assert_eq!(out[1].rect, [0.0, 0.0, 100.0, 2.5]);
        assert_eq!(out[1].paint[0], 11.0);
        assert_eq!(out[2].rect, [0.0, 45.0, 100.0, 5.0]);
        assert_eq!(out[2].paint[0], 12.0);
        assert!(out.iter().all(|instance| instance.params[3] == 6.0));
    }

    #[test]
    fn patterned_outline_scales_width_and_offset_without_emitting_layout_geometry() {
        let visual = VisualStyle {
            outline_width: Some(2.0),
            outline_offset: Some(3.0),
            outline_style: Some(BorderLineStyle::Dotted),
            outline_color: Some(ColorRef::Rgba([1.0, 0.0, 0.0, 1.0])),
            ..VisualStyle::default()
        };
        let mut out = Vec::new();

        emit_outline(
            &mut out,
            [10.0, 20.0, 50.0, 30.0],
            [4.0; 4],
            &visual,
            &Theme::dark(),
            2.0,
            None,
        );

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rect, [0.0, 10.0, 70.0, 50.0]);
        assert_eq!(out[0].paint[0], 10.0);
        assert_eq!(out[0].paint[3], 4.0);
        assert_eq!(out[0].params[2], 3.0);
    }

    #[test]
    fn border_transitions_interpolate_per_edge_widths_and_colors_but_not_styles() {
        let from = VisualStyle {
            border_top_width: Some(2.0),
            border_left_color: Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0])),
            border_style: Some(BorderLineStyle::Dotted),
            ..VisualStyle::default()
        };
        let to = VisualStyle {
            border_top_width: Some(6.0),
            border_left_color: Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0])),
            border_style: Some(BorderLineStyle::Dashed),
            ..VisualStyle::default()
        };
        let instant = to.clone();

        let interpolated = interpolate_visual_style(
            &from,
            &to,
            &instant,
            0.5,
            &Theme::dark(),
            Some(&[
                TransitionProperty::BorderWidth,
                TransitionProperty::BorderColor,
            ]),
        );

        assert_eq!(interpolated.border_top_width, Some(4.0));
        assert_eq!(
            interpolated.border_left_color,
            Some(ColorRef::Rgba([0.5, 0.5, 0.5, 1.0]))
        );
        assert_eq!(interpolated.border_style, Some(BorderLineStyle::Dashed));
    }

    #[test]
    fn retained_base_range_replacement_shifts_following_leaf_and_overlay_offsets() {
        let mut instances = (0..6)
            .map(|index| inst([index as f32, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0], 0.0))
            .collect::<Vec<_>>();
        let mut ranges = HashMap::from([("first".to_string(), 0..2), ("second".to_string(), 2..4)]);
        let mut overlay_start = 4;
        let replacement = (0..3)
            .map(|_| inst([9.0, 0.0, 1.0, 1.0], [0.0, 1.0, 0.0, 1.0], 0.0))
            .collect();

        assert!(replace_retained_base_range(
            &mut instances,
            &mut ranges,
            &mut overlay_start,
            "first",
            replacement,
        ));
        assert_eq!(ranges["first"], 0..3);
        assert_eq!(ranges["second"], 3..5);
        assert_eq!(overlay_start, 5);
        assert_eq!(instances.len(), 7);
        assert_eq!(instances[5].rect[0], 4.0);
    }

    #[test]
    fn retained_leaf_ranges_exclude_widgets_below_transformed_ancestors() {
        let mut root = node("root", WidgetKind::Panel);
        root.children.push(node("button", WidgetKind::Button));
        root.children.push(node("label", WidgetKind::Label));
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "root".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 160.0,
                h: 80.0,
            },
        );
        layout.rects.insert(
            "button".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 30.0,
            },
        );
        layout.rects.insert(
            "label".to_string(),
            Rect {
                x: 10.0,
                y: 48.0,
                w: 100.0,
                h: 20.0,
            },
        );
        let mut out = Vec::new();
        let mut ranges = HashMap::new();
        let mut icon_geometry_cache = IconGeometryCache::default();
        emit_rects_inner(
            &root,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            true,
            RenderContext::default(),
            &mut out,
            Some(&mut ranges),
            &mut icon_geometry_cache,
        );
        assert!(ranges.contains_key("button"));
        assert_eq!(ranges["label"].start, ranges["label"].end);

        root.style.visual.transform = Some(TransformStyle {
            translate_x: 5.0,
            ..TransformStyle::default()
        });
        out.clear();
        ranges.clear();
        emit_rects_inner(
            &root,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            true,
            RenderContext::default(),
            &mut out,
            Some(&mut ranges),
            &mut icon_geometry_cache,
        );
        assert!(!ranges.contains_key("button"));
        assert!(!ranges.contains_key("label"));
    }

    #[test]
    fn loading_spinner_phase_preserves_frame_time_at_epoch_scale() {
        let base = 1_800_000_000.0;
        let phase0 = loading_spinner_phase_at_seconds(1.0, base);
        let phase1 = loading_spinner_phase_at_seconds(1.0, base + 1.0 / 60.0);

        assert!(
            (phase1 - phase0).abs() > 0.01,
            "spinner phase should advance across one frame even with epoch-sized timestamps"
        );
    }

    #[test]
    fn text_area_selection_rect_honors_horizontal_scroll_when_unwrapped() {
        let mut area = node("notes", WidgetKind::TextArea);
        area.props.wrap = Some(false);
        area.style.text.font_size = Some(16.0);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "notes".to_string(),
            Rect {
                x: 10.0,
                y: 20.0,
                w: 240.0,
                h: 80.0,
            },
        );
        let theme = Theme::dark();
        let text = "iiii WWWW longer text".to_string();
        let start = text.find("WWWW").expect("selection start");
        let end = start + "WWWW".len();
        let carets = HashMap::new();

        let mut unscrolled = WidgetState::default();
        unscrolled
            .text_val
            .insert("notes".to_string(), text.clone());
        assert!(unscrolled.set_text_selection("notes", start, end));
        let mut unscrolled_out = Vec::new();
        emit_rects(
            &area,
            &layout,
            &theme,
            1.0,
            &unscrolled,
            &carets,
            &mut unscrolled_out,
        );

        let mut scrolled = WidgetState::default();
        scrolled.text_val.insert("notes".to_string(), text);
        assert!(scrolled.set_text_selection("notes", start, end));
        assert!(scrolled.scroll_text_area_with_max_scroll("notes", 12.0, 0.0, 0.0));
        let mut scrolled_out = Vec::new();
        emit_rects(
            &area,
            &layout,
            &theme,
            1.0,
            &scrolled,
            &carets,
            &mut scrolled_out,
        );

        let selection_color = |inst: &&RectInstance| (inst.color[3] - 0.34).abs() < 0.001;
        let unscrolled_rect = unscrolled_out
            .iter()
            .find(selection_color)
            .expect("unscrolled selection rect")
            .rect;
        let scrolled_rect = scrolled_out
            .iter()
            .find(selection_color)
            .expect("scrolled selection rect")
            .rect;

        assert!(
            (scrolled_rect[0] - (unscrolled_rect[0] - 12.0)).abs() < 0.5,
            "selection x should shift by scroll amount: unscrolled={unscrolled_rect:?} scrolled={scrolled_rect:?}"
        );
    }

    #[test]
    fn heatmap_scalar_bar_reserves_label_gutter() {
        let mut heatmap = node("heat", WidgetKind::Heatmap);
        heatmap.props.heatmap.rows = 28;
        heatmap.props.heatmap.cols = 36;
        heatmap.props.heatmap.values = vec![0.0; 28 * 36];
        heatmap.props.heatmap.vmin = -1.25;
        heatmap.props.heatmap.vmax = 2.75;
        heatmap.props.heatmap.show_labels = false;
        heatmap.props.heatmap.scalar_bar = true;

        let rect = [0.0, 0.0, 330.0, 220.0];
        let plot = heatmap_plot_rect(&heatmap, 1.0, rect);
        let bar = heatmap_scalar_bar_rect(&heatmap, 1.0, rect).expect("scalar bar rect");
        let scalar_gutter = rect[0] + rect[2] - (plot[0] + plot[2]);
        let label_x = bar[0] + bar[2] + 6.0;
        let label_room = rect[0] + rect[2] - label_x;

        assert!(
            scalar_gutter >= 80.0,
            "scalar bar gutter should reserve room for bar and labels, got {scalar_gutter}"
        );
        assert!(
            label_room >= 40.0,
            "scalar bar labels need enough room inside the widget rect, got {label_room}"
        );
    }

    #[test]
    fn heatmap_cell_stride_keeps_small_grids_exact() {
        let mut heatmap = node("heat-small", WidgetKind::Heatmap);
        heatmap.props.heatmap.rows = 32;
        heatmap.props.heatmap.cols = 32;
        heatmap.props.heatmap.values = vec![0.0; 32 * 32];
        heatmap.props.heatmap.show_labels = true;
        heatmap.props.heatmap.scalar_bar = true;

        let rect = [0.0, 0.0, 420.0, 280.0];
        let plot = heatmap_plot_rect(&heatmap, 1.0, rect);

        assert_eq!(heatmap_cell_stride(&heatmap, plot, 1.0), 1);
    }

    #[test]
    fn heatmap_cell_stride_downsamples_dense_pixel_grids() {
        let mut heatmap = node("heat-dense", WidgetKind::Heatmap);
        heatmap.props.heatmap.rows = 96;
        heatmap.props.heatmap.cols = 144;
        heatmap.props.heatmap.values = vec![0.0; 96 * 144];
        heatmap.props.heatmap.show_labels = false;
        heatmap.props.heatmap.scalar_bar = true;

        let rect = [0.0, 0.0, 720.0, 260.0];
        let plot = heatmap_plot_rect(&heatmap, 1.0, rect);

        assert_eq!(heatmap_cell_stride(&heatmap, plot, 1.0), 2);
    }

    #[test]
    fn emit_heatmap_limits_dense_cell_primitives() {
        let mut heatmap = node("heat-dense", WidgetKind::Heatmap);
        heatmap.props.heatmap.rows = 96;
        heatmap.props.heatmap.cols = 144;
        heatmap.props.heatmap.values = vec![0.0; 96 * 144];
        heatmap.props.heatmap.show_labels = false;
        heatmap.props.heatmap.scalar_bar = true;

        let mut out = Vec::new();
        emit_heatmap(
            &mut out,
            &heatmap,
            &Theme::dark(),
            1.0,
            [0.0, 0.0, 720.0, 260.0],
            None,
            None,
            [0.0; 4],
            1.0,
        );

        assert!(
            out.len() < 3_600,
            "dense heatmap should emit the strided cell grid, got {} rect instances",
            out.len()
        );
    }

    #[test]
    fn bar_chart_value_labels_auto_contrast_and_accept_style_part_color() {
        let mut chart = node("bars", WidgetKind::BarChart);
        chart.props.bar_chart.labels = vec!["A".to_string()];
        chart.props.bar_chart.series = vec![BarChartSeriesProp {
            label: Some("value".to_string()),
            values: vec![10.0],
            color: Some(ColorRef::Rgba([1.0, 0.92, 0.35, 1.0])),
        }];
        chart.props.bar_chart.show_axes = false;
        chart.props.bar_chart.show_ticks = false;
        chart.props.bar_chart.show_grid = false;

        let labels = bar_chart_text_labels(&chart, &Theme::dark(), 1.0, [0.0, 0.0, 220.0, 160.0]);
        let value = labels
            .iter()
            .find(|label| label.text == "10.00")
            .expect("bar value label");

        assert_eq!(value.color, Some([0.035, 0.045, 0.065]));

        chart.style.parts.parts.insert(
            "label".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(ColorRef::Rgba([0.18, 0.24, 0.32, 1.0])),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let labels = bar_chart_text_labels(&chart, &Theme::dark(), 1.0, [0.0, 0.0, 220.0, 160.0]);
        let value = labels
            .iter()
            .find(|label| label.text == "10.00")
            .expect("styled bar value label");

        assert_eq!(value.color, Some([0.18, 0.24, 0.32]));
        assert_eq!(value.font_size, Some(12.0));
    }

    #[test]
    fn pie_chart_labels_accept_style_part_color() {
        let mut chart = node("pie", WidgetKind::PieChart);
        chart.props.pie_chart.total = 4.0;
        chart.props.pie_chart.slices = vec![
            crate::document::PieChartSliceProp {
                label: "North".to_string(),
                value: 3.0,
                color: None,
            },
            crate::document::PieChartSliceProp {
                label: "South".to_string(),
                value: 1.0,
                color: None,
            },
        ];
        chart.props.pie_chart.show_labels = true;
        chart.props.pie_chart.label_mode = "inside".to_string();
        chart.style.parts.parts.insert(
            "label".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(ColorRef::Rgba([0.18, 0.24, 0.32, 1.0])),
                    font_size: Some(12.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let labels = pie_chart_text_labels(&chart, &Theme::dark(), 1.0, [0.0, 0.0, 260.0, 190.0]);
        let value = labels
            .iter()
            .find(|label| label.text == "North 75%")
            .expect("styled pie slice label");

        assert_eq!(value.color, Some([0.18, 0.24, 0.32]));
        assert_eq!(value.font_size, Some(12.0));
    }

    fn progress_bar_primitive_bench_fixture(
        count: usize,
    ) -> (WidgetNode, LayoutResult, WidgetState) {
        let mut root = node("root", WidgetKind::FlowLayout);
        root.children.reserve(count);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            root.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 420.0,
                h: (count as f32 * 24.0).max(1.0),
            },
        );

        for index in 0..count {
            let id = format!("progress-{index}");
            let mut bar = node(&id, WidgetKind::ProgressBar);
            let t = (index % 101) as f32 / 100.0;
            bar.props.value = Some(t);
            bar.props.min = Some(0.0);
            bar.props.max = Some(1.0);
            layout.rects.insert(
                id,
                Rect {
                    x: 0.0,
                    y: index as f32 * 24.0,
                    w: 360.0,
                    h: 18.0,
                },
            );
            root.children.push(bar);
        }

        let state = WidgetState::from_tree(&root);
        (root, layout, state)
    }

    #[test]
    fn range_and_progress_renderers_consume_shared_part_paint_fallbacks() {
        let theme = Theme::dark();

        let mut slider = node("slider", WidgetKind::Slider);
        slider.props.value = Some(0.5);
        slider.props.min = Some(0.0);
        slider.props.max = Some(1.0);
        let mut slider_layout = LayoutResult::default();
        slider_layout.rects.insert(
            slider.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 24.0,
            },
        );
        let slider_state = WidgetState::from_tree(&slider);
        let mut slider_out = Vec::new();
        emit_rects(
            &slider,
            &slider_layout,
            &theme,
            1.0,
            &slider_state,
            &HashMap::new(),
            &mut slider_out,
        );
        let slider_track = widget_part_paint_fallback(&slider, "track", &theme, &slider_state)
            .background
            .expect("slider track fallback");
        let slider_thumb = widget_part_paint_fallback(&slider, "thumb", &theme, &slider_state)
            .background
            .expect("slider thumb fallback");
        assert!(slider_out
            .iter()
            .any(|instance| instance.color == slider_track));
        assert!(slider_out
            .iter()
            .any(|instance| instance.color == slider_thumb));

        let mut progress = node("progress", WidgetKind::ProgressBar);
        progress.props.value = Some(0.5);
        progress.props.min = Some(0.0);
        progress.props.max = Some(1.0);
        let mut progress_layout = LayoutResult::default();
        progress_layout.rects.insert(
            progress.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 20.0,
            },
        );
        let progress_state = WidgetState::from_tree(&progress);
        let mut progress_out = Vec::new();
        emit_rects(
            &progress,
            &progress_layout,
            &theme,
            1.0,
            &progress_state,
            &HashMap::new(),
            &mut progress_out,
        );
        let progress_track =
            widget_part_paint_fallback(&progress, "track", &theme, &progress_state)
                .background
                .expect("progress track fallback");
        let progress_fill = widget_part_paint_fallback(&progress, "fill", &theme, &progress_state)
            .background
            .expect("progress fill fallback");
        assert!(progress_out
            .iter()
            .any(|instance| instance.color == progress_track));
        assert!(progress_out
            .iter()
            .any(|instance| instance.color == progress_fill));
    }

    #[test]
    fn navigation_renderers_consume_shared_selected_part_colors() {
        let theme = Theme::dark();

        let tree = node("tree-node", WidgetKind::TreeNode);
        let mut tree_layout = LayoutResult::default();
        tree_layout.rects.insert(
            tree.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 28.0,
            },
        );
        let mut tree_state = WidgetState::from_tree(&tree);
        tree_state.selectable_selected.insert(tree.id.clone(), true);
        let mut tree_out = Vec::new();
        emit_rects(
            &tree,
            &tree_layout,
            &theme,
            1.0,
            &tree_state,
            &HashMap::new(),
            &mut tree_out,
        );
        let tree_fill = widget_part_paint_fallback(&tree, "row", &theme, &tree_state)
            .background
            .expect("selected tree row fallback");
        assert!(tree_out.iter().any(|instance| instance.color == tree_fill));

        let tab = node("tab-a", WidgetKind::Tab);
        let mut tab_layout = LayoutResult::default();
        tab_layout.rects.insert(
            tab.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 36.0,
            },
        );
        let mut tab_state = WidgetState::default();
        tab_state
            .tab_parent
            .insert(tab.id.clone(), "tabs".to_string());
        tab_state.tab_values.insert(tab.id.clone(), "a".to_string());
        tab_state
            .active_tabs
            .insert("tabs".to_string(), "a".to_string());
        let mut tab_out = Vec::new();
        emit_rects(
            &tab,
            &tab_layout,
            &theme,
            1.0,
            &tab_state,
            &HashMap::new(),
            &mut tab_out,
        );
        let tab_fill = widget_part_paint_fallback(&tab, "tab", &theme, &tab_state)
            .background
            .expect("active tab fallback");
        let tab_accent = widget_part_paint_fallback(&tab, "accent", &theme, &tab_state)
            .background
            .expect("active tab accent fallback");
        assert!(tab_out.iter().any(|instance| instance.color == tab_fill));
        assert!(tab_out.iter().any(|instance| instance.color == tab_accent));

        let nav = node("nav-overview", WidgetKind::NavItem);
        let mut nav_layout = LayoutResult::default();
        nav_layout.rects.insert(
            nav.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 36.0,
            },
        );
        let mut nav_state = WidgetState::default();
        nav_state
            .nav_targets
            .insert(nav.id.clone(), "overview".to_string());
        nav_state
            .page_owner
            .insert("overview".to_string(), "pages".to_string());
        nav_state
            .active_pages
            .insert("pages".to_string(), "overview".to_string());
        let mut nav_out = Vec::new();
        emit_rects(
            &nav,
            &nav_layout,
            &theme,
            1.0,
            &nav_state,
            &HashMap::new(),
            &mut nav_out,
        );
        let nav_fill = widget_part_paint_fallback(&nav, "item", &theme, &nav_state)
            .background
            .expect("active nav item fallback");
        let nav_accent = widget_part_paint_fallback(&nav, "accent", &theme, &nav_state)
            .background
            .expect("active nav accent fallback");
        assert!(nav_out.iter().any(|instance| instance.color == nav_fill));
        assert!(nav_out.iter().any(|instance| instance.color == nav_accent));
    }

    #[test]
    fn data_widget_renderers_consume_shared_container_and_table_part_colors() {
        let theme = Theme::dark();

        let bars = node("bars", WidgetKind::BarChart);
        let mut bars_layout = LayoutResult::default();
        bars_layout.rects.insert(
            bars.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 140.0,
            },
        );
        let bars_state = WidgetState::from_tree(&bars);
        let mut bars_out = Vec::new();
        emit_rects(
            &bars,
            &bars_layout,
            &theme,
            1.0,
            &bars_state,
            &HashMap::new(),
            &mut bars_out,
        );
        let bars_fallback = widget_paint_fallback(&bars, &theme, &bars_state);
        assert!(bars_out
            .iter()
            .any(|instance| instance.color
                == bars_fallback.background.expect("chart surface fallback")));
        assert!(bars_out
            .iter()
            .any(|instance| instance.color
                == bars_fallback.border_color.expect("chart border fallback")));

        let mut heatmap = node("heatmap", WidgetKind::Heatmap);
        heatmap.props.heatmap.rows = 2;
        heatmap.props.heatmap.cols = 2;
        heatmap.props.heatmap.values = vec![0.0, 0.25, 0.5, 1.0];
        heatmap.props.heatmap.finite_count = 4;
        heatmap.props.heatmap.scalar_bar = true;
        heatmap.props.heatmap.hover = Some(HeatmapHoverProp {
            row: 0,
            col: 1,
            value: 0.25,
            screen: [110.0, 70.0],
            x_label: None,
            y_label: None,
        });
        let mut heatmap_layout = LayoutResult::default();
        heatmap_layout.rects.insert(
            heatmap.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 260.0,
                h: 180.0,
            },
        );
        let heatmap_state = WidgetState::from_tree(&heatmap);
        let mut heatmap_out = Vec::new();
        emit_rects(
            &heatmap,
            &heatmap_layout,
            &theme,
            1.0,
            &heatmap_state,
            &HashMap::new(),
            &mut heatmap_out,
        );
        for (part, property) in [
            ("grid", "background"),
            ("scalar-bar", "border-color"),
            ("hover", "background"),
            ("hover", "border-color"),
        ] {
            let fallback = native_widget_part_paint_fallback(
                WidgetKind::Heatmap,
                part,
                &theme,
                if part == "hover" {
                    PaintInteraction::Hovered
                } else {
                    PaintInteraction::Resting
                },
                false,
            );
            let color = match property {
                "background" => fallback.background,
                "border-color" => fallback.border_color,
                _ => None,
            }
            .expect("heatmap part fallback");
            assert!(
                heatmap_out.iter().any(|instance| instance.color == color),
                "missing cataloged Heatmap::{part} {property}"
            );
        }
        for (part, background, border) in [
            ("cell", Some(rgba(0.3, 0.2, 0.7)), None),
            ("grid", Some(rgba(0.9, 0.1, 0.2)), None),
            (
                "scalar-bar",
                Some(rgba(0.2, 0.3, 0.4)),
                Some(rgba(0.4, 0.5, 0.6)),
            ),
            (
                "hover",
                Some(rgba(0.1, 0.8, 0.3)),
                Some(rgba(0.8, 0.9, 0.2)),
            ),
        ] {
            heatmap.style.parts.parts.insert(
                part.to_string(),
                PartStyle {
                    visual: VisualStyle {
                        background,
                        border_color: border,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
        }
        heatmap_out.clear();
        emit_rects(
            &heatmap,
            &heatmap_layout,
            &theme,
            1.0,
            &heatmap_state,
            &HashMap::new(),
            &mut heatmap_out,
        );
        for color in [
            [0.3, 0.2, 0.7, 1.0],
            [0.9, 0.1, 0.2, 1.0],
            [0.2, 0.3, 0.4, 1.0],
            [0.4, 0.5, 0.6, 1.0],
            [0.1, 0.8, 0.3, 1.0],
            [0.8, 0.9, 0.2, 1.0],
        ] {
            assert!(
                heatmap_out.iter().any(|instance| instance.color == color),
                "missing authored Heatmap part color {color:?}"
            );
        }

        let mut table = node("table", WidgetKind::DataFrameTable);
        table.props.table_rows = Some(2);
        table.props.page_size = Some(2);
        table.props.table_columns = vec!["value".to_string()];
        table.props.table_dtypes = vec!["f64".to_string()];
        let mut table_layout = LayoutResult::default();
        table_layout.rects.insert(
            table.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 100.0,
            },
        );
        let mut table_state = WidgetState::from_tree(&table);
        assert!(table_state.select_table_cell(&table.id, 0, 0));
        let mut table_out = Vec::new();
        emit_rects(
            &table,
            &table_layout,
            &theme,
            1.0,
            &table_state,
            &HashMap::new(),
            &mut table_out,
        );
        for part in ["header", "row-selected", "grid-line"] {
            let color = widget_part_paint_fallback(&table, part, &theme, &table_state)
                .background
                .expect("table part fallback");
            assert!(
                table_out.iter().any(|instance| instance.color == color),
                "missing cataloged DataFrameTable::{part} color"
            );
        }
    }

    #[test]
    fn progress_bar_small_fill_uses_stable_radius_with_soft_cutoff() {
        let mut bar = node("progress", WidgetKind::ProgressBar);
        bar.props.value = Some(0.03);
        bar.props.min = Some(0.0);
        bar.props.max = Some(1.0);
        bar.style.parts.parts.insert(
            "fill".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.0, 1.0, 0.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "progress".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 20.0,
            },
        );
        let state = WidgetState::from_tree(&bar);
        let mut out = Vec::new();

        emit_rects(
            &bar,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|instance| instance.color == [0.0, 1.0, 0.0, 1.0])
            .expect("styled progress fill primitive");

        assert_eq!(fill.rect, [3.0, 3.0, 194.0, 14.0]);
        assert_eq!(fill.clip, default_local_clip(fill.rect));
        // Fill corners are concentric with the compact track: the dark theme's
        // 3px radius minus the 3px inset resolves to square inner corners.
        assert_eq!(fill.radii, [0.0; 4]);
        assert!(
            (fill.paint[3] - 5.82).abs() < 0.01,
            "small progress should use a soft cutoff at the progress width, got paint={:?}",
            fill.paint
        );
        assert_eq!(fill.params[3], 5.0);
    }

    fn table_primitive_bench_fixture() -> (WidgetNode, LayoutResult, WidgetState, usize) {
        let rows = env_usize("DRAGONGUI_TABLE_BENCH_ROWS", 100_000);
        let cols = env_usize("DRAGONGUI_TABLE_BENCH_COLS", 64);
        let width = env_usize("DRAGONGUI_TABLE_BENCH_WIDTH", 1200) as f32;
        let height = env_usize("DRAGONGUI_TABLE_BENCH_HEIGHT", 800) as f32;

        let mut table = node("table", WidgetKind::DataFrameTable);
        table.props.table_rows = Some(rows);
        table.props.page_size = Some(rows);
        table.props.table_columns = (0..cols).map(|index| format!("col_{index}")).collect();
        table.props.table_dtypes = (0..cols).map(|_| "f64".to_string()).collect();

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            table.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: height,
            },
        );

        let state = WidgetState::from_tree(&table);
        let visible = state
            .table("table")
            .map(|state| {
                let metrics = table::metrics_for_node(&table, &Theme::dark(), 1.0);
                let rect = layout.rects.get("table").copied().unwrap();
                let visible = table::visible(state, &rect, metrics);
                visible.row_count * visible.col_count
            })
            .unwrap_or(0);
        (table, layout, state, visible)
    }

    fn line_plot_bench_fixture(
        series_count: usize,
        points_per_series: usize,
    ) -> (WidgetNode, LayoutResult) {
        let mut plot = node("line-plot", WidgetKind::LinePlot);
        plot.props.line_plot_line_width = 2.0;
        plot.props.line_plot_auto_fit = false;
        plot.props.line_plot_series.reserve(series_count);
        for series_index in 0..series_count {
            let mut points = Vec::with_capacity(points_per_series);
            for index in 0..points_per_series {
                let x = index as f32;
                let phase = series_index as f32 * 0.37;
                let y = ((index as f32 * 0.006) + phase).sin()
                    + ((index as f32 * 0.0017) + phase * 0.5).cos() * 0.35;
                points.push([x, y]);
            }
            plot.props.line_plot_series.push(LinePlotSeriesProp {
                label: Some(format!("series {series_index}")),
                color: None,
                line_style: "solid".to_string(),
                points,
                front_offset: 0,
                y_blocks: Vec::new(),
                bounds: Some([0.0, points_per_series.saturating_sub(1) as f32, -1.5, 1.5]),
                x_sorted: true,
                payload_format: LinePlotPayloadFormat::XyF32V0,
                declared_point_count: Some(points_per_series),
            });
        }

        let mut root = node("root", WidgetKind::Panel);
        root.children.push(plot);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "root".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1280.0,
                h: 720.0,
            },
        );
        layout.rects.insert(
            "line-plot".to_string(),
            Rect {
                x: 24.0,
                y: 24.0,
                w: 1200.0,
                h: 640.0,
            },
        );
        (root, layout)
    }

    fn env_usize(name: &str, default: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(default)
    }

    fn rgba(r: f32, g: f32, b: f32) -> ColorRef {
        ColorRef::Rgba([r, g, b, 1.0])
    }

    fn has_rect(out: &[RectInstance], color: [f32; 4], rect: [f32; 4]) -> bool {
        out.iter()
            .any(|inst| inst.color == color && inst.rect == rect)
    }

    #[test]
    fn simple_path_accepts_solid_rounded_rect_instances() {
        let instance = inst_radii([4.0, 5.0, 120.0, 36.0], [0.1, 0.2, 0.3, 0.8], [8.0; 4]);

        assert!(is_simple_rect_instance(&instance));
    }

    #[test]
    fn retained_patch_pipeline_routing_respects_split_modes() {
        let simple = inst_radii([4.0, 5.0, 120.0, 36.0], [0.1, 0.2, 0.3, 0.8], [8.0; 4]);
        let mut line = inst_radii([10.0, 20.0, 160.0, 3.0], [0.2, 0.4, 0.8, 1.0], [1.5; 4]);
        line.transform2[0] = 0.42;
        line.transform2[3] = 1.0;
        let mut complex = simple;
        complex.paint[0] = 1.0;

        assert_eq!(
            pipeline_kind_for_instance(&simple, true, false),
            PrimitivePipelineKind::Simple
        );
        assert_eq!(
            pipeline_kind_for_instance(&line, true, false),
            PrimitivePipelineKind::Line
        );
        assert_eq!(
            pipeline_kind_for_instance(&complex, true, false),
            PrimitivePipelineKind::Complex
        );
        assert_eq!(
            pipeline_kind_for_instance(&simple, false, false),
            PrimitivePipelineKind::Complex
        );
        assert_eq!(
            pipeline_kind_for_instance(&line, true, true),
            PrimitivePipelineKind::Complex
        );
    }

    #[test]
    fn retained_patch_upload_span_covers_sparse_indices() {
        let mut range = None;
        include_instance_index(&mut range, 7);
        include_instance_index(&mut range, 3);
        include_instance_index(&mut range, 5);

        assert_eq!(range, Some(3..8));
    }

    #[test]
    #[ignore]
    fn bench_progress_bar_primitive_emit() {
        let count = env_usize("DRAGONGUI_PROGRESS_BENCH_COUNT", 10_000);
        let iterations = env_usize("DRAGONGUI_PROGRESS_BENCH_ITERS", 200);
        let warmup = env_usize("DRAGONGUI_PROGRESS_BENCH_WARMUP", 20);
        let (tree, layout, state) = progress_bar_primitive_bench_fixture(count);
        let theme = Theme::dark();
        let caret_positions = HashMap::new();
        let mut out = Vec::with_capacity(count * 3);

        for _ in 0..warmup {
            out.clear();
            emit_rects(
                &tree,
                &layout,
                &theme,
                1.0,
                &state,
                &caret_positions,
                &mut out,
            );
        }

        let start = Instant::now();
        let mut emitted = 0usize;
        for _ in 0..iterations {
            out.clear();
            emit_rects(
                &tree,
                &layout,
                &theme,
                1.0,
                &state,
                &caret_positions,
                &mut out,
            );
            emitted += out.len();
        }
        let elapsed = start.elapsed();
        let bars = count * iterations;
        let ns_per_bar = elapsed.as_nanos() as f64 / bars as f64;
        let emitted_per_bar = emitted as f64 / bars as f64;
        eprintln!(
            "progress primitive emit: count={count} iterations={iterations} total_ms={:.3} ns_per_bar={:.1} emitted_per_bar={:.2}",
            elapsed.as_secs_f64() * 1000.0,
            ns_per_bar,
            emitted_per_bar
        );
    }

    #[test]
    #[ignore]
    fn bench_table_primitive_emit() {
        let iterations = env_usize("DRAGONGUI_TABLE_BENCH_ITERS", 1_000);
        let warmup = env_usize("DRAGONGUI_TABLE_BENCH_WARMUP", 50);
        let (tree, layout, state, visible_cells) = table_primitive_bench_fixture();
        let theme = Theme::dark();
        let caret_positions = HashMap::new();
        let mut out = Vec::with_capacity(256);

        for _ in 0..warmup {
            out.clear();
            emit_rects(
                &tree,
                &layout,
                &theme,
                1.0,
                &state,
                &caret_positions,
                &mut out,
            );
        }

        let start = Instant::now();
        let mut emitted = 0usize;
        for _ in 0..iterations {
            out.clear();
            emit_rects(
                &tree,
                &layout,
                &theme,
                1.0,
                &state,
                &caret_positions,
                &mut out,
            );
            emitted += out.len();
        }
        let elapsed = start.elapsed();
        let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
        let ns_per_visible_cell = if visible_cells == 0 {
            0.0
        } else {
            elapsed.as_nanos() as f64 / (iterations * visible_cells) as f64
        };
        eprintln!(
            "table primitive emit: iterations={iterations} visible_cells={visible_cells} total_ms={:.3} ns_per_iter={:.1} ns_per_visible_cell={:.1} emitted_per_iter={:.2}",
            elapsed.as_secs_f64() * 1000.0,
            ns_per_iter,
            ns_per_visible_cell,
            emitted as f64 / iterations as f64
        );
    }

    #[test]
    #[ignore]
    fn bench_line_plot_render_data_collect() {
        let series_count = env_usize("DRAGONGUI_LINE_PLOT_BENCH_SERIES", 4);
        let points_per_series = env_usize("DRAGONGUI_LINE_PLOT_BENCH_POINTS", 100_000);
        let iterations = env_usize("DRAGONGUI_LINE_PLOT_BENCH_ITERS", 100);
        let warmup = env_usize("DRAGONGUI_LINE_PLOT_BENCH_WARMUP", 10);
        let max_segments = env_usize("DRAGONGUI_LINE_PLOT_BENCH_MAX_SEGMENTS", 8_192);
        let (tree, layout) = line_plot_bench_fixture(series_count, points_per_series);
        let theme = Theme::dark();
        let mut points = Vec::new();
        let mut series_instances = Vec::new();

        for _ in 0..warmup {
            points.clear();
            series_instances.clear();
            let mut build_stats = LinePlotRenderBuildStats::default();
            collect_line_plot_render_data(
                &tree,
                &layout,
                &theme,
                1.0,
                1.0,
                max_segments,
                LinePlotDecimationMode::Auto,
                None,
                &mut points,
                &mut series_instances,
                &mut build_stats,
            );
            std::hint::black_box((&points, &series_instances, build_stats));
        }

        let start = Instant::now();
        let mut emitted_points = 0usize;
        let mut emitted_series = 0usize;
        let mut source_points = 0usize;
        let mut decimated_series = 0u32;
        let mut decimate_ms = 0.0f64;
        for _ in 0..iterations {
            points.clear();
            series_instances.clear();
            let mut build_stats = LinePlotRenderBuildStats::default();
            collect_line_plot_render_data(
                &tree,
                &layout,
                &theme,
                1.0,
                1.0,
                max_segments,
                LinePlotDecimationMode::Auto,
                None,
                &mut points,
                &mut series_instances,
                &mut build_stats,
            );
            emitted_points += points.len();
            emitted_series += series_instances.len();
            source_points += build_stats.source_point_count;
            decimated_series += build_stats.decimated_series_count;
            decimate_ms += build_stats.decimate_ms;
            std::hint::black_box((&points, &series_instances));
        }
        let elapsed = start.elapsed();
        let total_source_points = iterations * series_count * points_per_series;
        eprintln!(
            "line plot render data collect: series={series_count} points_per_series={points_per_series} iterations={iterations} total_ms={:.3} ns_per_source_point={:.2} emitted_points_per_iter={:.1} series_runs_per_iter={:.1} source_points_per_iter={:.1} decimated_series_per_iter={:.1} measured_decimate_ms={:.3}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / total_source_points as f64,
            emitted_points as f64 / iterations as f64,
            emitted_series as f64 / iterations as f64,
            source_points as f64 / iterations as f64,
            decimated_series as f64 / iterations as f64,
            decimate_ms
        );
    }

    #[test]
    fn simple_path_rejects_featureful_rect_instances() {
        let solid = inst_radii([4.0, 5.0, 120.0, 36.0], [0.1, 0.2, 0.3, 1.0], [8.0; 4]);
        let mut transformed = solid;
        transformed.transform[0] = 2.0;
        let shadow = inst_shadow_clipped(
            [4.0, 5.0, 120.0, 36.0],
            [0.0, 0.0, 0.0, 0.4],
            [8.0; 4],
            8.0,
            default_local_clip([4.0, 5.0, 120.0, 36.0]),
        );
        let gradient = inst_linear_gradient(
            [4.0, 5.0, 120.0, 36.0],
            [
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
                [0.0; 4],
                [0.0; 4],
                [0.0; 4],
                [0.0; 4],
            ],
            [0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            2.0,
            0.0,
            [8.0; 4],
            90.0,
        );

        assert!(!is_simple_rect_instance(&transformed));
        assert!(!is_simple_rect_instance(&shadow));
        assert!(!is_simple_rect_instance(&gradient));
    }

    #[test]
    fn line_path_accepts_rotated_solid_capsules() {
        let mut line = inst_radii([10.0, 20.0, 160.0, 3.0], [0.2, 0.4, 0.8, 1.0], [1.5; 4]);
        line.transform2[0] = 0.42;
        line.transform2[3] = 1.0;
        let mut short_line = inst_radii([10.0, 20.0, 1.2, 3.0], [0.2, 0.4, 0.8, 1.0], [1.5; 4]);
        short_line.transform2[0] = 0.42;
        short_line.transform2[3] = 1.0;

        assert!(is_line_segment_instance(&line));
        assert!(is_line_segment_instance(&short_line));
        assert!(!is_simple_rect_instance(&line));
    }

    #[test]
    fn line_fast_path_preserves_ancestor_paint_clip_in_screen_space() {
        let mut line = inst_radii([10.0, 20.0, 160.0, 3.0], [0.2, 0.4, 0.8, 1.0], [1.5; 4]);
        line.transform2[0] = 0.42;
        line.transform2[3] = 1.0;
        line.clip = [20.0, 1.0, 60.0, 2.0];

        let compact = line_segment_instance_from_rect(line);

        assert_eq!(compact.clip, [30.0, 21.0, 70.0, 22.0]);
    }

    #[test]
    fn line_path_rejects_featureful_capsules() {
        let mut axis_aligned = inst_radii([10.0, 20.0, 160.0, 3.0], [0.2, 0.4, 0.8, 1.0], [1.5; 4]);
        axis_aligned.transform2[3] = 1.0;
        let mut gradient = axis_aligned;
        gradient.transform2[0] = 0.42;
        gradient.paint[0] = 1.0;
        let mut scaled = axis_aligned;
        scaled.transform2[0] = 0.42;
        scaled.transform[2] = 1.2;

        assert!(is_line_segment_instance(&axis_aligned));
        assert!(!is_line_segment_instance(&gradient));
        assert!(!is_line_segment_instance(&scaled));
    }

    #[test]
    fn line_join_dots_only_emit_for_visible_turns() {
        let plot = [0.0, 0.0, 100.0, 80.0];
        let color = [0.2, 0.4, 0.8, 1.0];
        let mut out = Vec::new();

        push_line_join_if_needed(
            &mut out,
            Some([0.0, 40.0]),
            [20.0, 40.0],
            [40.0, 40.5],
            plot,
            2.0,
            color,
        );
        assert!(out.is_empty());

        push_line_join_if_needed(
            &mut out,
            Some([0.0, 40.0]),
            [20.0, 40.0],
            [20.0, 60.0],
            plot,
            2.0,
            color,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].transform2[3], 1.0);
        assert!(is_line_segment_instance(&out[0]));
    }

    #[test]
    fn line_plot_decimation_preserves_bucket_extrema() {
        let visible: Vec<[f32; 2]> = (0..100)
            .map(|i| {
                let y = match i {
                    17 => 10.0,
                    72 => -8.0,
                    _ => (i as f32 * 0.1).sin(),
                };
                [i as f32, y]
            })
            .collect();
        let bounds = LinePlotBounds {
            x_min: 0.0,
            x_max: 99.0,
            y_min: -8.0,
            y_max: 10.0,
        };
        let mut points = Vec::new();
        let mut series_instances = Vec::new();
        let mut run_offset = 0usize;
        let mut run_len = 0usize;
        let mut last_mapped = None;
        let mut path_distance = 0.0;

        let decimated = push_decimated_line_plot_renderer_series(
            &mut points,
            &mut series_instances,
            &visible,
            [0.0, 0.0, 10.0, 80.0],
            [0.0, 0.0, 10.0, 80.0],
            bounds,
            2.0,
            [0.2, 0.4, 0.8, 1.0],
            1.0,
            64,
            LinePlotDecimationMode::Extrema,
            0.0,
            true,
            &mut run_offset,
            &mut run_len,
            &mut last_mapped,
            &mut path_distance,
        );
        flush_line_plot_renderer_run(
            &mut points,
            &mut series_instances,
            &mut run_offset,
            &mut run_len,
            [0.0, 0.0, 10.0, 80.0],
            [0.0, 0.0, 10.0, 80.0],
            bounds,
            2.0,
            [0.2, 0.4, 0.8, 1.0],
            1.0,
            0.0,
        );

        let ys: Vec<f32> = points.iter().map(|point| point.data[1]).collect();
        assert!(decimated);
        assert!(points.len() < visible.len());
        assert!(series_instances.len() == 1);
        assert!(ys.contains(&10.0));
        assert!(ys.contains(&-8.0));
    }

    #[test]
    fn line_plot_auto_decimation_uses_pixel_stride() {
        let visible: Vec<[f32; 2]> = (0..100).map(|i| [i as f32, i as f32]).collect();
        let bounds = LinePlotBounds {
            x_min: 0.0,
            x_max: 99.0,
            y_min: 0.0,
            y_max: 99.0,
        };
        let mut points = Vec::new();
        let mut series_instances = Vec::new();
        let mut run_offset = 0usize;
        let mut run_len = 0usize;
        let mut last_mapped = None;
        let mut path_distance = 0.0;

        let decimated = push_decimated_line_plot_renderer_series(
            &mut points,
            &mut series_instances,
            &visible,
            [0.0, 0.0, 10.0, 80.0],
            [0.0, 0.0, 10.0, 80.0],
            bounds,
            2.0,
            [0.2, 0.4, 0.8, 1.0],
            1.0,
            64,
            LinePlotDecimationMode::Auto,
            0.0,
            true,
            &mut run_offset,
            &mut run_len,
            &mut last_mapped,
            &mut path_distance,
        );

        assert!(decimated);
        assert!(points.len() < visible.len());
        assert_eq!(points.first().map(|point| point.data[0]), Some(0.0));
        assert_eq!(points.last().map(|point| point.data[0]), Some(99.0));
        assert!(
            points.len() <= line_plot_fast_decimation_target_points([0.0, 0.0, 10.0, 80.0], 64)
        );
    }

    #[test]
    fn split_batch_pressure_guard_only_trips_on_many_runs() {
        assert!(!should_collapse_split_batches(3_000, 1, 0));
        assert!(!should_collapse_split_batches(3_000, 512, 0));
        assert!(should_collapse_split_batches(5_643, 5_016, 0));
        assert!(!should_collapse_split_batches(2_000, 1_600, 1_200));
    }

    #[test]
    fn styled_box_shadow_emits_soft_shadow_instance_before_surface() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.box_shadows = Some(vec![BoxShadow {
            offset_x: 2.0,
            offset_y: 4.0,
            blur: 6.0,
            spread: 1.0,
            color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.25]),
            inset: false,
        }]);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 30.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let shadow = out.first().expect("shadow instance");
        assert_eq!(shadow.rect, [5.0, 7.0, 114.0, 44.0]);
        assert_eq!(shadow.color, [0.0, 0.0, 0.0, 0.25]);
        assert_eq!(shadow.params, [6.0, 6.0, 1.0, 0.0]);
    }

    #[test]
    fn narrow_button_badge_pill_rect_stays_inside_parent() {
        let mut button = node("narrow", WidgetKind::Button);
        button.props.badge = Some("owner: platform-design".to_string());

        let parent = [10.0, 12.0, 42.0, 28.0];
        let badge = badge_rect(&button, parent, &Theme::dark(), 1.0, 8.0)
            .expect("narrow badge should still produce a clipped pill rect");

        assert!(
            badge[0] >= parent[0] && badge[0] + badge[2] <= parent[0] + parent[2],
            "badge pill should stay inside parent: parent={parent:?} badge={badge:?}"
        );
        assert!(
            badge[2] <= parent[2] - 8.0,
            "badge pill width should be capped by available parent space: {badge:?}"
        );
    }

    #[test]
    fn narrow_tab_and_nav_badge_pill_rects_stay_inside_parent() {
        let theme = Theme::dark();
        for kind in [WidgetKind::Tab, WidgetKind::NavItem] {
            let mut node = node("narrow", kind);
            node.props.badge = Some("overflow-count".to_string());
            let parent = [4.0, 6.0, 36.0, 26.0];
            let badge = badge_rect(&node, parent, &theme, 1.0, 8.0)
                .expect("narrow inline badge should still produce a clipped pill rect");

            assert!(
                badge[0] >= parent[0] && badge[0] + badge[2] <= parent[0] + parent[2],
                "{kind:?} badge pill should stay inside parent: parent={parent:?} badge={badge:?}"
            );
        }
    }

    #[test]
    fn disabled_button_still_emits_inline_badge_pill_inside_parent() {
        let mut button = node("disabled", WidgetKind::Button);
        button.props.badge = Some("1234567890".to_string());
        button.props.disabled = true;

        let parent = Rect {
            x: 10.0,
            y: 12.0,
            w: 54.0,
            h: 30.0,
        };
        let mut layout = LayoutResult::default();
        layout.rects.insert("disabled".to_string(), parent);
        let theme = Theme::dark();
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &theme,
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let expected = badge_rect(
            &button,
            [parent.x, parent.y, parent.w, parent.h],
            &theme,
            1.0,
            theme.spacing,
        )
        .expect("disabled badge rect");
        assert!(
            out.iter().any(|inst| inst.rect == expected),
            "disabled button should emit badge pill at {expected:?}; emitted rects: {:?}",
            out.iter().map(|inst| inst.rect).collect::<Vec<_>>()
        );
        let emitted_badge = out
            .iter()
            .find(|inst| {
                let [x, y, w, h] = inst.rect;
                x >= parent.x
                    && y >= parent.y
                    && x + w <= parent.x + parent.w
                    && y + h <= parent.y + parent.h
                    && w > 0.0
                    && h > 0.0
                    && w < parent.w
                    && h < parent.h
            })
            .expect("disabled button should emit a positive, contained badge-sized rect");
        assert!(
            emitted_badge.rect[2] > 1.0 && emitted_badge.rect[3] > 1.0,
            "disabled badge rect should have visible area: {:?}",
            emitted_badge.rect
        );
    }

    #[test]
    fn backdrop_filter_brightness_and_saturation_affect_tint_instance() {
        let mut out = Vec::new();
        emit_backdrop_filter_tint(
            &mut out,
            [10.0, 12.0, 100.0, 40.0],
            [8.0; 4],
            BackdropFilterStyle {
                blur: 12.0,
                brightness: 1.2,
                saturate: 1.3,
            },
        );

        let tint = out.first().expect("backdrop tint instance");
        assert_eq!(tint.rect, [10.0, 12.0, 100.0, 40.0]);
        assert_eq!(tint.radii, [8.0; 4]);
        assert_eq!(tint.color[0], 0.92);
        assert_eq!(tint.color[1], 0.97);
        assert_eq!(tint.color[2], 1.0);
        assert!((tint.color[3] - 0.09416667).abs() < 0.0001);
    }

    #[test]
    fn solid_outline_preserves_square_corners() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.border_radius = Some(0.0);
        button.style.visual.outline_color = Some(ColorRef::Rgba([0.10, 0.20, 0.30, 0.40]));
        button.style.visual.outline_width = Some(2.0);
        button.style.visual.outline_offset = Some(3.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 30.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let outline: Vec<_> = out
            .iter()
            .filter(|inst| inst.color == [0.10, 0.20, 0.30, 0.40])
            .collect();
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].rect, [5.0, 5.0, 110.0, 40.0]);
        assert_eq!(outline[0].radii, [0.0; 4]);
        assert_eq!(outline[0].clip, [-1.0, -1.0, 111.0, 41.0]);
        assert_eq!(outline[0].params[2], 3.0);
        assert_eq!(outline[0].paint[3], 2.0);
    }

    #[test]
    fn solid_outline_expands_rounded_corners() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.border_radius = Some(8.0);
        button.style.visual.outline_color = Some(ColorRef::Rgba([0.10, 0.20, 0.30, 0.40]));
        button.style.visual.outline_width = Some(2.0);
        button.style.visual.outline_offset = Some(3.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 30.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let outline: Vec<_> = out
            .iter()
            .filter(|inst| inst.color == [0.10, 0.20, 0.30, 0.40])
            .collect();
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].rect, [5.0, 5.0, 110.0, 40.0]);
        assert_eq!(outline[0].radii, [13.0; 4]);
        assert_eq!(outline[0].params[2], 3.0);
        assert_eq!(outline[0].paint[3], 2.0);
    }

    #[test]
    fn bordered_rounded_rect_uses_ring_border() {
        let mut out = Vec::new();

        emit_bordered_rect_radii(
            &mut out,
            [10.0, 12.0, 100.0, 40.0],
            [0.1, 0.2, 0.3, 1.0],
            [0.4, 0.5, 0.6, 1.0],
            [9.0, 8.0, 7.0, 6.0],
            2.0,
        );

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].rect, [12.0, 14.0, 96.0, 36.0]);
        assert_eq!(out[0].radii, [7.0, 6.0, 5.0, 4.0]);
        assert_eq!(out[1].rect, [10.0, 12.0, 100.0, 40.0]);
        assert_eq!(out[1].radii, [9.0, 8.0, 7.0, 6.0]);
        assert_eq!(out[1].params[2], 3.0);
        assert_eq!(out[1].paint[3], 2.0);
    }

    #[test]
    fn zero_width_border_paints_fill_without_ring() {
        let mut out = Vec::new();

        emit_bordered_rect_radii(
            &mut out,
            [10.0, 12.0, 100.0, 40.0],
            [0.1, 0.2, 0.3, 1.0],
            [0.4, 0.5, 0.6, 1.0],
            [9.0, 8.0, 7.0, 6.0],
            0.0,
        );

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rect, [10.0, 12.0, 100.0, 40.0]);
        assert_eq!(out[0].radii, [9.0, 8.0, 7.0, 6.0]);
        assert_eq!(out[0].color, [0.4, 0.5, 0.6, 1.0]);
    }

    #[test]
    fn clipped_panel_keeps_full_paint_rect_and_uses_local_clip() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background = Some(ColorRef::Rgba([0.4, 0.5, 0.6, 1.0]));
        panel.style.visual.border_color = Some(ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]));
        panel.style.visual.border_width = Some(2.0);
        panel.style.visual.border_radius = Some(8.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 10.0,
                y: 20.0,
                w: 120.0,
                h: 80.0,
            },
        );
        layout.clips.insert(
            "panel".to_string(),
            Rect {
                x: 10.0,
                y: 42.0,
                w: 120.0,
                h: 58.0,
            },
        );
        layout.paint_clips.insert(
            "panel".to_string(),
            Rect {
                x: 10.0,
                y: 42.0,
                w: 120.0,
                h: 58.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.color == [0.4, 0.5, 0.6, 1.0])
            .expect("panel fill should be emitted");
        assert_eq!(fill.rect, [12.0, 22.0, 116.0, 76.0]);
        assert_eq!(fill.clip, [-1.0, 19.0, 117.0, 77.0]);

        let border = out
            .iter()
            .find(|inst| inst.color == [0.1, 0.2, 0.3, 1.0])
            .expect("panel border should be emitted");
        assert_eq!(border.rect, [10.0, 20.0, 120.0, 80.0]);
        assert_eq!(border.clip, [-1.0, 21.0, 121.0, 81.0]);
    }

    #[test]
    fn titled_panel_header_and_body_parts_paint_their_owned_geometry() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.props.text = Some("Structural parts".to_string());
        panel.style.layout.width = Some(240.0);
        panel.style.layout.height = Some(140.0);
        panel.style.visual.border_width = Some(2.0);
        panel.style.visual.border_radius = Some(10.0);

        let mut header = PartStyle::default();
        header.visual.background = Some(ColorRef::Rgba([0.8, 0.1, 0.2, 1.0]));
        header.visual.border_color = Some(ColorRef::Rgba([0.2, 0.8, 0.1, 1.0]));
        header.visual.border_width = Some(1.0);
        panel.style.parts.parts.insert("header".to_string(), header);

        let mut body = PartStyle::default();
        body.visual.background = Some(ColorRef::Rgba([0.1, 0.2, 0.8, 1.0]));
        panel.style.parts.parts.insert("body".to_string(), body);

        let mut root = node("window", WidgetKind::Window);
        root.children.push(panel);
        let theme = Theme::dark();
        let layout = crate::layout::compute_layout(&root, 320.0, 220.0, 1.0, &theme, None);
        let panel = &root.children[0];
        let geometry =
            titled_container_geometry(panel, &layout, 1.0, &theme).expect("titled panel geometry");
        let mut out = Vec::new();

        emit_rects(
            &root,
            &layout,
            &theme,
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let header_fill = out
            .iter()
            .find(|instance| instance.color == [0.8, 0.1, 0.2, 1.0])
            .expect("header part fill");
        assert_eq!(
            header_fill.rect,
            [
                geometry.title_band.x + 1.0,
                geometry.title_band.y + 1.0,
                geometry.title_band.w - 2.0,
                geometry.title_band.h - 2.0,
            ]
        );
        let header_border = out
            .iter()
            .find(|instance| instance.color == [0.2, 0.8, 0.1, 1.0])
            .expect("header part border");
        assert_eq!(
            header_border.rect,
            [
                geometry.title_band.x,
                geometry.title_band.y,
                geometry.title_band.w,
                geometry.title_band.h,
            ]
        );
        let body_fill = out
            .iter()
            .find(|instance| instance.color == [0.1, 0.2, 0.8, 1.0])
            .expect("body part fill");
        assert_eq!(
            body_fill.rect,
            [
                geometry.body_viewport.x,
                geometry.body_viewport.y,
                geometry.body_viewport.w,
                geometry.body_viewport.h,
            ]
        );
        assert_eq!(header_fill.radii[2..], [0.0, 0.0]);
        assert_eq!(body_fill.radii[..2], [0.0, 0.0]);
    }

    #[test]
    fn untitled_panel_does_not_paint_header_or_body_virtual_parts() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.layout.width = Some(200.0);
        panel.style.layout.height = Some(100.0);
        for (part, color) in [
            ("header", [0.8, 0.1, 0.2, 1.0]),
            ("body", [0.1, 0.2, 0.8, 1.0]),
        ] {
            let mut style = PartStyle::default();
            style.visual.background = Some(ColorRef::Rgba(color));
            panel.style.parts.parts.insert(part.to_string(), style);
        }
        let mut root = node("window", WidgetKind::Window);
        root.children.push(panel);
        let theme = Theme::dark();
        let layout = crate::layout::compute_layout(&root, 240.0, 160.0, 1.0, &theme, None);
        let mut out = Vec::new();

        emit_rects(
            &root,
            &layout,
            &theme,
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        assert!(!out.iter().any(|instance| {
            instance.color == [0.8, 0.1, 0.2, 1.0] || instance.color == [0.1, 0.2, 0.8, 1.0]
        }));
    }

    #[test]
    fn fully_visible_panel_keeps_antialias_clip_pad() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background = Some(ColorRef::Rgba([0.4, 0.5, 0.6, 1.0]));
        panel.style.visual.border_color = Some(ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]));
        panel.style.visual.border_width = Some(2.0);
        panel.style.visual.border_radius = Some(8.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 20.0,
                y: 24.0,
                w: 120.0,
                h: 80.0,
            },
        );
        layout.clips.insert(
            "panel".to_string(),
            Rect {
                x: 20.0,
                y: 24.0,
                w: 120.0,
                h: 80.0,
            },
        );
        layout.paint_clips.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 300.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let border = out
            .iter()
            .find(|inst| inst.color == [0.1, 0.2, 0.3, 1.0])
            .expect("panel border should be emitted");
        assert_eq!(border.rect, [20.0, 24.0, 120.0, 80.0]);
        assert_eq!(border.clip, [-1.0, -1.0, 121.0, 81.0]);
    }

    #[test]
    fn child_flush_with_paint_clip_keeps_left_antialias_pad() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.background = Some(ColorRef::Rgba([0.4, 0.5, 0.6, 1.0]));
        button.style.visual.border_color = Some(ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]));
        button.style.visual.border_width = Some(1.0);
        button.style.visual.border_radius = Some(7.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 30.0,
                y: 40.0,
                w: 110.0,
                h: 34.0,
            },
        );
        layout.clips.insert(
            "run".to_string(),
            Rect {
                x: 30.0,
                y: 40.0,
                w: 110.0,
                h: 34.0,
            },
        );
        layout.paint_clips.insert(
            "run".to_string(),
            Rect {
                x: 30.0,
                y: 40.0,
                w: 110.0,
                h: 34.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let border = out
            .iter()
            .find(|inst| inst.color == [0.1, 0.2, 0.3, 1.0])
            .expect("button border should be emitted");
        assert_eq!(border.clip[0], -1.0);
        assert_eq!(border.clip[2], 111.0);
    }

    #[test]
    fn relative_positioned_widget_clips_against_painted_offset() {
        let mut badge = node("badge", WidgetKind::Badge);
        badge.style.layout.position = Some(PositionStyle::Relative);
        badge.style.layout.top = Some(18.0);
        badge.style.visual.background = Some(ColorRef::Rgba([0.4, 0.5, 0.6, 1.0]));
        badge.style.visual.border_color = Some(ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]));
        badge.style.visual.border_width = Some(1.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "badge".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 40.0,
            },
        );
        layout.clips.insert(
            "badge".to_string(),
            Rect {
                x: 10.0,
                y: 30.0,
                w: 100.0,
                h: 20.0,
            },
        );
        layout.paint_clips.insert(
            "badge".to_string(),
            Rect {
                x: 0.0,
                y: 30.0,
                w: 200.0,
                h: 80.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &badge,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let border = out
            .iter()
            .find(|inst| inst.color == [0.1, 0.2, 0.3, 1.0])
            .expect("badge border should be emitted");
        assert_eq!(border.transform[1], 18.0);
        assert_eq!(border.clip[1], 1.0);
    }

    #[test]
    fn collapsible_border_ring_paints_after_header_fill() {
        let mut collapsible = node("advanced", WidgetKind::Collapsible);
        collapsible.props.expanded = Some(true);
        collapsible.style.visual.background = Some(ColorRef::Rgba([0.04, 0.05, 0.06, 1.0]));
        collapsible.style.visual.border_color = Some(ColorRef::Rgba([0.10, 0.70, 0.30, 1.0]));
        collapsible.style.visual.border_width = Some(2.0);
        collapsible.style.visual.border_radius = Some(8.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "advanced".to_string(),
            Rect {
                x: 10.0,
                y: 12.0,
                w: 180.0,
                h: 88.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &collapsible,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let header_fill_index = out
            .iter()
            .position(|inst| inst.color == Theme::dark().surface_alt)
            .expect("collapsible header fill should be emitted");
        let border_index = out
            .iter()
            .position(|inst| {
                inst.color == [0.10, 0.70, 0.30, 1.0]
                    && inst.params[2] == 3.0
                    && inst.paint[3] == 2.0
            })
            .expect("collapsible border ring should be emitted");

        assert!(border_index > header_fill_index);
    }

    #[test]
    fn collapsible_renderer_consumes_shared_surface_and_header_paint() {
        let theme = Theme::dark();
        let mut collapsible = node("advanced", WidgetKind::Collapsible);
        collapsible.props.expanded = Some(true);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            collapsible.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 120.0,
            },
        );

        let state = WidgetState::from_tree(&collapsible);
        let mut out = Vec::new();
        emit_rects(
            &collapsible,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );
        let surface = widget_paint_fallback(&collapsible, &theme, &state);
        let header = widget_part_paint_fallback(&collapsible, "header", &theme, &state);
        for color in [
            surface.background.expect("collapsible surface"),
            surface.border_color.expect("collapsible border"),
            header.background.expect("collapsible header"),
        ] {
            assert!(out.iter().any(|instance| instance.color == color));
        }

        let mut hovered = state;
        hovered.hovered = Some(collapsible.id.clone());
        out.clear();
        emit_rects(
            &collapsible,
            &layout,
            &theme,
            1.0,
            &hovered,
            &HashMap::new(),
            &mut out,
        );
        let hovered_header = widget_part_paint_fallback(&collapsible, "header", &theme, &hovered)
            .background
            .expect("hovered collapsible header");
        assert!(out.iter().any(|instance| instance.color == hovered_header));
    }

    #[test]
    fn disclosure_renderers_consume_shared_mark_colors() {
        let theme = Theme::dark();
        for (kind, part, height) in [
            (WidgetKind::Collapsible, "indicator", 48.0),
            (WidgetKind::Dropdown, "chevron", 32.0),
        ] {
            let disclosure = node(part, kind);
            let mut layout = LayoutResult::default();
            layout.rects.insert(
                disclosure.id.clone(),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 160.0,
                    h: height,
                },
            );

            for interaction in [PaintInteraction::Resting, PaintInteraction::Disabled] {
                let mut state = WidgetState::from_tree(&disclosure);
                if interaction == PaintInteraction::Disabled {
                    state.disabled.insert(disclosure.id.clone());
                }
                let mut out = Vec::new();
                emit_rects(
                    &disclosure,
                    &layout,
                    &theme,
                    1.0,
                    &state,
                    &HashMap::new(),
                    &mut out,
                );
                let color =
                    native_widget_part_paint_fallback(kind, part, &theme, interaction, false)
                        .background
                        .expect("disclosure mark fallback");
                assert!(
                    out.iter().any(|instance| instance.color == color),
                    "missing cataloged {interaction:?} {kind:?}::{part} mark"
                );
            }
        }
    }

    #[test]
    fn custom_icon_resource_emits_tinted_segments() {
        let resource = serde_json::json!({
            "type": "stroke",
            "view_box": [0, 0, 24, 24],
            "stroke_width": 2,
            "strokes": [
                {"points": [[4, 4], [20, 20]]},
                {"points": [[20, 4], [4, 20]], "closed": false}
            ]
        });
        let color = [0.2, 0.4, 0.6, 1.0];
        let mut instances = Vec::new();

        let geometry = parse_custom_icon_resource(&resource).expect("valid icon resource");
        emit_custom_icon_geometry(&mut instances, &geometry, [10.0, 20.0, 40.0, 32.0], color);
        assert_eq!(instances.len(), 2);
        assert!(instances.iter().all(|instance| instance.color == color));
    }

    #[test]
    fn client_restore_icon_is_smaller_and_rounded_without_changing_copy_icon() {
        fn bounds(instances: &[RectInstance]) -> [f32; 4] {
            let left = instances
                .iter()
                .map(|instance| instance.rect[0])
                .fold(f32::INFINITY, f32::min);
            let top = instances
                .iter()
                .map(|instance| instance.rect[1])
                .fold(f32::INFINITY, f32::min);
            let right = instances
                .iter()
                .map(|instance| instance.rect[0] + instance.rect[2])
                .fold(f32::NEG_INFINITY, f32::max);
            let bottom = instances
                .iter()
                .map(|instance| instance.rect[1] + instance.rect[3])
                .fold(f32::NEG_INFINITY, f32::max);
            [left, top, right - left, bottom - top]
        }

        let rect = [0.0, 0.0, 46.0, 34.0];
        let color = [0.9, 0.9, 0.9, 1.0];
        let mut generic = Vec::new();
        emit_tool_copy_icon(&mut generic, rect, color, 1.0);

        let mut restore = node("restore", WidgetKind::IconButton);
        restore.css_types.push("WindowMaximize".to_string());
        restore
            .props
            .raw_props
            .insert("icon".to_string(), serde_json::json!("copy"));
        let mut restored = Vec::new();
        emit_tool_icon_button_mark(
            &mut restored,
            &restore,
            rect,
            color,
            1.0,
            &mut IconGeometryCache::default(),
        );

        assert_eq!(
            restored.len(),
            2,
            "restore should use two rounded window rings"
        );
        assert!(restored.iter().all(|instance| {
            instance.radii.iter().all(|radius| *radius > 0.0) && instance.paint[3] > 0.0
        }));
        let generic_bounds = bounds(&generic);
        let restore_bounds = bounds(&restored);
        assert!(restore_bounds[2] < generic_bounds[2]);
        assert!(restore_bounds[3] < generic_bounds[3]);
        assert!((restore_bounds[0] + restore_bounds[2] * 0.5 - rect[2] * 0.5).abs() <= 0.5);
        assert!((restore_bounds[1] + restore_bounds[3] * 0.5 - rect[3] * 0.5).abs() <= 0.5);
    }

    #[test]
    fn action_button_renderers_consume_shared_icon_mark_colors() {
        let theme = Theme::dark();
        for kind in [WidgetKind::IconButton, WidgetKind::ArrowButton] {
            let button = node("action", kind);
            let mut layout = LayoutResult::default();
            layout.rects.insert(
                button.id.clone(),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 32.0,
                    h: 32.0,
                },
            );

            for interaction in [PaintInteraction::Resting, PaintInteraction::Disabled] {
                let mut state = WidgetState::from_tree(&button);
                if interaction == PaintInteraction::Disabled {
                    state.disabled.insert(button.id.clone());
                }
                let mut out = Vec::new();
                emit_rects(
                    &button,
                    &layout,
                    &theme,
                    1.0,
                    &state,
                    &HashMap::new(),
                    &mut out,
                );
                let color =
                    native_widget_part_paint_fallback(kind, "icon", &theme, interaction, false)
                        .background
                        .expect("action icon fallback");
                assert!(
                    out.iter().any(|instance| instance.color == color),
                    "missing cataloged {interaction:?} {kind:?}::icon mark"
                );
            }
        }
    }

    #[test]
    fn structural_divider_renderers_consume_separator_and_splitter_catalog_paint() {
        let theme = Theme::dark();
        let separator = node("rule", WidgetKind::Separator);
        let mut separator_layout = LayoutResult::default();
        separator_layout.rects.insert(
            separator.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 1.0,
            },
        );
        let mut separator_out = Vec::new();
        emit_rects(
            &separator,
            &separator_layout,
            &theme,
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut separator_out,
        );
        let separator_color = widget_paint_fallback(&separator, &theme, &WidgetState::default())
            .background
            .expect("separator fallback");
        assert!(separator_out
            .iter()
            .any(|instance| instance.color == separator_color));

        let mut splitter = node("split", WidgetKind::Splitter);
        splitter.children = vec![
            node("left", WidgetKind::Pane),
            node("right", WidgetKind::Pane),
        ];
        let mut splitter_layout = LayoutResult::default();
        splitter_layout.rects.insert(
            splitter.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 206.0,
                h: 120.0,
            },
        );
        splitter_layout.rects.insert(
            "left".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        splitter_layout.rects.insert(
            "right".to_string(),
            Rect {
                x: 106.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        let splitter_state = WidgetState::from_tree(&splitter);
        let mut splitter_out = Vec::new();
        emit_rects(
            &splitter,
            &splitter_layout,
            &theme,
            1.0,
            &splitter_state,
            &HashMap::new(),
            &mut splitter_out,
        );
        let gutter_color = widget_part_paint_fallback(&splitter, "gutter", &theme, &splitter_state)
            .background
            .expect("splitter gutter fallback");
        assert!(splitter_out
            .iter()
            .any(|instance| instance.color == gutter_color));
    }

    #[test]
    fn clipped_box_shadow_keeps_full_shape_and_uses_inherited_paint_clip() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.box_shadows = Some(vec![BoxShadow {
            offset_x: 2.0,
            offset_y: 4.0,
            blur: 6.0,
            spread: 1.0,
            color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.25]),
            inset: false,
        }]);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 90.0,
                w: 100.0,
                h: 40.0,
            },
        );
        layout.clips.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 90.0,
                w: 100.0,
                h: 20.0,
            },
        );
        layout.paint_clips.insert(
            "run".to_string(),
            Rect {
                x: 0.0,
                y: 50.0,
                w: 200.0,
                h: 60.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let shadow = out.first().expect("shadow instance");
        assert_eq!(shadow.rect, [5.0, 87.0, 114.0, 54.0]);
        assert_eq!(shadow.clip, [-1.0, -1.0, 115.0, 24.0]);
    }

    #[test]
    fn multiple_box_shadows_emit_multiple_shadow_instances() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.box_shadows = Some(vec![
            BoxShadow {
                offset_x: 0.0,
                offset_y: 2.0,
                blur: 4.0,
                spread: 0.0,
                color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.18]),
                inset: false,
            },
            BoxShadow {
                offset_x: 0.0,
                offset_y: 10.0,
                blur: 12.0,
                spread: 2.0,
                color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.24]),
                inset: false,
            },
        ]);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 30.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let shadows: Vec<_> = out.iter().filter(|inst| inst.params[2] == 1.0).collect();
        assert_eq!(shadows.len(), 2);
        assert_eq!(shadows[0].rect, [6.0, 8.0, 108.0, 38.0]);
        assert_eq!(shadows[0].color, [0.0, 0.0, 0.0, 0.18]);
        assert_eq!(shadows[1].rect, [-4.0, 6.0, 128.0, 58.0]);
        assert_eq!(shadows[1].color, [0.0, 0.0, 0.0, 0.24]);
    }

    #[test]
    fn inset_box_shadow_emits_inner_shadow_instance_after_surface() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.box_shadows = Some(vec![BoxShadow {
            offset_x: 0.0,
            offset_y: 2.0,
            blur: 8.0,
            spread: 1.0,
            color: ColorRef::Rgba([1.0, 1.0, 1.0, 0.20]),
            inset: true,
        }]);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 30.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let inset_index = out
            .iter()
            .position(|inst| inst.params[2] == 2.0)
            .expect("inset shadow instance");
        assert!(
            inset_index > 1,
            "inset shadow should render after the button surface"
        );
        let shadow = &out[inset_index];
        assert_eq!(shadow.rect, [10.0, 10.0, 100.0, 30.0]);
        assert_eq!(shadow.color, [1.0, 1.0, 1.0, 0.20]);
        assert_eq!(shadow.params, [8.0, 0.0, 2.0, 0.0]);
        assert_eq!(shadow.paint, [0.0, 0.0, 2.0, 1.0]);
    }

    #[test]
    fn linear_gradient_background_emits_gradient_rect_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 180.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 0.0, 0.0, 1.0]),
                        position: None,
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 1.0, 1.0]),
                        position: None,
                    },
                ],
            }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 1.0)
            .expect("gradient fill instance");
        assert_eq!(fill.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(fill.color2, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(fill.paint[0], 1.0);
        assert_eq!(fill.paint[3], 2.0);
        assert_eq!(fill.gradient_stops, [0.0, 1.0, 1.0, 1.0]);
        assert!((fill.paint[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn extension_display_list_emits_scaled_primitives() {
        let mut extension = node("paint", WidgetKind::Extension);
        let props = serde_json::json!({
            "extension_type": "paint",
            "paint_width": 100,
            "paint_height": 50,
            "display_list": [
                {"cmd": "rect", "x": 10, "y": 5, "w": 20, "h": 10, "fill": [255, 0, 0, 255], "radius": 2},
                {"cmd": "line", "x1": 0, "y1": 0, "x2": 100, "y2": 50, "stroke": "accent", "stroke_width": 2},
                {"cmd": "circle", "cx": 50, "cy": 25, "r": 4, "fill": "success"}
            ]
        });
        extension.props.raw_props = props.as_object().unwrap().clone();
        extension.props.extension_type = Some("paint".to_string());
        extension.props.intrinsic_width = Some(100.0);
        extension.props.intrinsic_height = Some(50.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "paint".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 100.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &extension,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let red_rect = out
            .iter()
            .find(|inst| inst.color == [1.0, 0.0, 0.0, 1.0])
            .expect("display-list rect");
        assert_eq!(red_rect.rect, [20.0, 10.0, 40.0, 20.0]);
        assert_eq!(red_rect.radii, [4.0, 4.0, 4.0, 4.0]);
        assert!(
            out.iter()
                .any(|inst| (inst.transform2[3] - 1.0).abs() < 0.001),
            "display-list line should emit a line segment primitive"
        );
        assert!(
            out.iter()
                .any(|inst| inst.color == Theme::dark().success
                    && inst.rect == [92.0, 42.0, 16.0, 16.0]),
            "display-list circle should resolve theme token colors"
        );
    }

    #[test]
    fn extension_display_list_lines_keep_scroll_ancestor_paint_clip() {
        let mut extension = node("scope", WidgetKind::Extension);
        let props = serde_json::json!({
            "extension_type": "scope",
            "paint_width": 100,
            "paint_height": 50,
            "display_list": [
                {"cmd": "line", "x1": 0, "y1": 0, "x2": 100, "y2": 50, "stroke": "accent", "stroke_width": 2}
            ]
        });
        extension.props.raw_props = props.as_object().unwrap().clone();
        extension.props.extension_type = Some("scope".to_string());
        extension.props.intrinsic_width = Some(100.0);
        extension.props.intrinsic_height = Some(50.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "scope".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 100.0,
            },
        );
        layout.paint_clips.insert(
            "scope".to_string(),
            Rect {
                x: 0.0,
                y: 40.0,
                w: 200.0,
                h: 20.0,
            },
        );
        let mut out = Vec::new();
        emit_rects(
            &extension,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let line = out
            .iter()
            .copied()
            .find(is_line_segment_instance)
            .expect("display-list line");
        let compact = line_segment_instance_from_rect(line);
        assert!(compact.clip[1] >= 39.0, "{:?}", compact.clip);
        assert!(compact.clip[3] <= 61.0, "{:?}", compact.clip);
    }

    #[test]
    fn multi_stop_linear_gradient_emits_stop_data() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 90.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 0.0, 0.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 0.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 1.0, 0.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 25.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 1.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 100.0,
                            px: 0.0,
                        }),
                    },
                ],
            }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 1.0)
            .expect("gradient fill instance");
        assert_eq!(fill.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(fill.color2, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(fill.color3, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(fill.color4, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(fill.gradient_stops, [0.0, 0.25, 1.0, 1.0]);
        assert_eq!(fill.paint[3], 3.0);
    }

    #[test]
    fn six_stop_linear_gradient_emits_extended_stop_data() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 90.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 0.0, 0.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 0.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 0.5, 0.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 18.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 0.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 34.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 1.0, 0.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 52.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 1.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 76.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.5, 0.0, 1.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 100.0,
                            px: 0.0,
                        }),
                    },
                ],
            }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 1.0)
            .expect("gradient fill instance");
        assert_eq!(fill.color5, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(fill.color6, [0.5, 0.0, 1.0, 1.0]);
        assert_eq!(fill.gradient_stops, [0.0, 0.18, 0.34, 0.52]);
        assert_eq!(fill.gradient_stops2[0], 0.76);
        assert_eq!(fill.gradient_stops2[1], 1.0);
        assert_eq!(fill.paint[3], 6.0);
    }

    #[test]
    fn gradient_interpolation_reaches_rect_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.gradient_interpolation = Some(GradientInterpolation::Oklab);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 90.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 0.0, 0.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 0.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 1.0, 1.0]),
                        position: Some(CalcLength {
                            percent: 100.0,
                            px: 0.0,
                        }),
                    },
                ],
            }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 1.0)
            .expect("gradient fill instance");
        assert_eq!(fill.transform2[2], 2.0);
    }

    #[test]
    fn blob_gradient_reaches_rect_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.gradient_interpolation = Some(GradientInterpolation::Oklab);
        panel.style.visual.background_paint = Some(BackgroundPaint::BlobGradient(BlobGradient {
            blobs: vec![
                BlobGradientStop {
                    center: [0.2, 0.3],
                    radius: 0.42,
                    color: ColorRef::Rgba([1.0, 0.0, 0.0, 0.5]),
                },
                BlobGradientStop {
                    center: [0.8, 0.4],
                    radius: 0.38,
                    color: ColorRef::Rgba([0.0, 0.0, 1.0, 0.45]),
                },
            ],
        }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 3.0)
            .expect("blob gradient fill instance");
        assert_eq!(fill.paint[3], 2.0);
        assert_eq!(fill.gradient_stops, [0.2, 0.3, 0.8, 0.4]);
        assert_eq!(fill.color5[0], 0.42);
        assert_eq!(fill.color5[1], 0.38);
        assert_eq!(fill.transform2[2], 2.0);
    }

    #[test]
    fn mesh_gradient_reaches_rect_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.gradient_interpolation = Some(GradientInterpolation::Oklab);
        panel.style.visual.background_paint = Some(BackgroundPaint::MeshGradient(MeshGradient {
            top_left: ColorRef::Rgba([0.1, 0.2, 0.8, 1.0]),
            top_right: ColorRef::Rgba([0.8, 0.2, 0.5, 1.0]),
            bottom_left: ColorRef::Rgba([0.1, 0.7, 0.5, 1.0]),
            bottom_right: ColorRef::Rgba([0.05, 0.08, 0.14, 1.0]),
        }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 4.0)
            .expect("mesh gradient fill instance");
        assert_eq!(fill.color, [0.1, 0.2, 0.8, 1.0]);
        assert_eq!(fill.color2, [0.8, 0.2, 0.5, 1.0]);
        assert_eq!(fill.color3, [0.1, 0.7, 0.5, 1.0]);
        assert_eq!(fill.color4, [0.05, 0.08, 0.14, 1.0]);
        assert_eq!(fill.transform2[2], 2.0);
    }

    #[test]
    fn gradient_sampling_uses_premultiplied_alpha() {
        let stops = vec![([1.0, 0.0, 0.0, 1.0], 0.0), ([0.0, 0.0, 0.0, 0.0], 1.0)];

        let color = gradient_color_at(&stops, 0.5);

        assert!((color[0] - 1.0).abs() < 0.0001);
        assert!((color[1] - 0.0).abs() < 0.0001);
        assert!((color[2] - 0.0).abs() < 0.0001);
        assert!((color[3] - 0.5).abs() < 0.0001);
    }

    #[test]
    fn repeating_linear_gradient_marks_negative_stop_count() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 90.0,
                repeating: true,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 1.0, 0.18]),
                        position: Some(CalcLength {
                            percent: 0.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 1.0, 0.18]),
                        position: Some(CalcLength {
                            percent: 8.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]),
                        position: Some(CalcLength {
                            percent: 8.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]),
                        position: Some(CalcLength {
                            percent: 16.0,
                            px: 0.0,
                        }),
                    },
                ],
            }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 1.0)
            .expect("repeating gradient fill instance");
        assert_eq!(fill.paint[3], -4.0);
        assert_eq!(fill.gradient_stops, [0.0, 0.08, 0.08, 0.16]);
    }

    #[test]
    fn pixel_gradient_period_remains_fixed_across_sizes_and_dpi() {
        let stops = vec![
            ResolvedGradientStop {
                color: [1.0, 1.0, 1.0, 1.0],
                position: Some(CalcLength {
                    percent: 0.0,
                    px: 0.0,
                }),
            },
            ResolvedGradientStop {
                color: [1.0, 1.0, 1.0, 1.0],
                position: Some(CalcLength {
                    percent: 0.0,
                    px: 1.0,
                }),
            },
            ResolvedGradientStop {
                color: [0.0, 0.0, 0.0, 1.0],
                position: Some(CalcLength {
                    percent: 0.0,
                    px: 2.0,
                }),
            },
        ];

        let (_, small, _) = prepare_gradient_stops(&stops, 100.0, 1.0);
        let (_, large, _) = prepare_gradient_stops(&stops, 200.0, 1.0);
        let (_, high_dpi, _) = prepare_gradient_stops(&stops, 200.0, 2.0);

        assert_eq!([small[1], small[2]], [0.01, 0.02]);
        assert_eq!([large[1], large[2]], [0.005, 0.01]);
        assert_eq!([high_dpi[1], high_dpi[2]], [0.01, 0.02]);
        assert_eq!(small[2] * 100.0, large[2] * 200.0);
        assert_eq!(high_dpi[2] * 200.0, 4.0);
    }

    #[test]
    fn mixed_gradient_positions_resolve_then_apply_css_fixup() {
        let stops = vec![
            ResolvedGradientStop {
                color: [1.0; 4],
                position: Some(CalcLength {
                    percent: -10.0,
                    px: 0.0,
                }),
            },
            ResolvedGradientStop {
                color: [0.8; 4],
                position: Some(CalcLength {
                    percent: 10.0,
                    px: 2.0,
                }),
            },
            ResolvedGradientStop {
                color: [0.4; 4],
                position: None,
            },
            ResolvedGradientStop {
                color: [0.0; 4],
                position: Some(CalcLength {
                    percent: 5.0,
                    px: 0.0,
                }),
            },
        ];

        let normalized = normalize_gradient_stops(&stops, 100.0, 2.0);
        let positions = normalized
            .iter()
            .map(|(_, position)| *position)
            .collect::<Vec<_>>();

        assert_eq!(positions, vec![0.0, 0.14, 0.14, 0.14]);
    }

    #[test]
    fn percentage_gradient_positions_are_independent_of_size_and_scale() {
        let stops = vec![
            ResolvedGradientStop {
                color: [1.0; 4],
                position: Some(CalcLength {
                    percent: 25.0,
                    px: 0.0,
                }),
            },
            ResolvedGradientStop {
                color: [0.0; 4],
                position: Some(CalcLength {
                    percent: 75.0,
                    px: 0.0,
                }),
            },
        ];

        let narrow = normalize_gradient_stops(&stops, 80.0, 1.0);
        let wide_high_dpi = normalize_gradient_stops(&stops, 640.0, 2.0);
        assert_eq!(narrow[0].1, 0.25);
        assert_eq!(narrow[1].1, 0.75);
        assert_eq!(narrow, wide_high_dpi);
    }

    #[test]
    fn layered_gradient_background_emits_back_to_front_instances() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint = Some(BackgroundPaint::Layers(vec![
            BackgroundPaint::RadialGradient(RadialGradient {
                repeating: false,
                center: [0.2, 0.25],
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 1.0, 0.18]),
                        position: Some(CalcLength {
                            percent: 0.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]),
                        position: Some(CalcLength {
                            percent: 65.0,
                            px: 0.0,
                        }),
                    },
                ],
            }),
            BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 135.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]),
                        position: Some(CalcLength {
                            percent: 0.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.1, 1.0]),
                        position: Some(CalcLength {
                            percent: 100.0,
                            px: 0.0,
                        }),
                    },
                ],
            }),
        ]));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let paints: Vec<f32> = out
            .iter()
            .filter_map(|inst| (inst.paint[0] > 0.5).then_some(inst.paint[0]))
            .collect();
        assert_eq!(paints, vec![1.0, 2.0]);
    }

    #[test]
    fn background_noise_reaches_rect_instances() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 135.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]),
                        position: Some(CalcLength {
                            percent: 0.0,
                            px: 0.0,
                        }),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.1, 1.0]),
                        position: Some(CalcLength {
                            percent: 100.0,
                            px: 0.0,
                        }),
                    },
                ],
            }));
        panel.style.visual.background_noise = Some(0.035);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 1.0)
            .expect("gradient fill instance");
        assert_eq!(fill.transform2[1], 0.035);
    }

    #[test]
    fn panel_scrollbar_stays_inside_rounded_panel_surface() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.props.text = Some("Controls".to_string());
        panel.style.visual.border_radius = Some(20.0);
        panel.style.visual.border_width = Some(1.0);
        panel.style.layout.padding = Some(18.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        layout.scroll_max_y.insert("panel".to_string(), 120.0);
        layout.scroll_y.insert("panel".to_string(), 0.0);
        let state = WidgetState::default();
        let theme = Theme::dark();
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let scrollbar: Vec<_> = out
            .iter()
            .filter(|inst| (inst.rect[2] - 4.0).abs() < 0.01)
            .collect();
        assert_eq!(scrollbar.len(), 2, "track and thumb should be emitted");
        let track = scrollbar[0];
        let thumb = scrollbar[1];
        assert!(
            track.rect[0] + track.rect[2] <= 92.0,
            "scrollbar should be inset from the rounded right edge: {:?}",
            track.rect
        );
        let top_gap = track.rect[1];
        let bottom_gap = 120.0 - (track.rect[1] + track.rect[3]);
        let title_inset = panel_scrollbar_title_inset(&panel, &layout, &theme, 1.0);
        assert!(
            top_gap >= title_inset,
            "titled panel scrollbar should start in the body area: {:?}",
            track.rect
        );
        assert!(
            bottom_gap >= 11.0,
            "scrollbar track should leave enough bottom breathing room: {:?}",
            track.rect
        );
        assert!(thumb.rect[1] >= track.rect[1]);
        assert!(thumb.rect[1] + thumb.rect[3] <= track.rect[1] + track.rect[3]);
    }

    #[test]
    fn titled_panel_scrollbar_track_stays_inside_body_with_styled_padding() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.props.text = Some("Spacer behavior".to_string());
        panel.style.visual.border_radius = Some(14.0);
        panel.style.visual.border_width = Some(1.0);
        panel.style.layout.padding = Some(14.0);
        panel.style.parts.parts.insert(
            "scrollbar-track".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(8.0),
                    padding: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        panel.style.parts.parts.insert(
            "scrollbar-thumb".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(6.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 240.0,
                h: 180.0,
            },
        );
        layout.scroll_max_y.insert("panel".to_string(), 80.0);
        layout.scroll_y.insert("panel".to_string(), 0.0);

        let geometry = panel_scrollbar_geometry(
            &panel,
            &layout,
            &WidgetState::default(),
            &Theme::dark(),
            1.0,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 240.0,
                h: 180.0,
            },
        )
        .expect("scrollbar geometry");
        let vertical = geometry.vertical.expect("vertical scrollbar");
        let title_inset = panel_scrollbar_title_inset(&panel, &layout, &Theme::dark(), 1.0);

        assert!(
            vertical.track.y >= title_inset,
            "track should not overlap title: track={:?} title_inset={title_inset}",
            vertical.track
        );
        assert!(
            vertical.track.y + vertical.track.h <= 180.0,
            "track should not overhang panel bottom: {:?}",
            vertical.track
        );
        assert!(
            180.0 - (vertical.track.y + vertical.track.h) >= 5.0,
            "track should keep bottom breathing room: {:?}",
            vertical.track
        );
        let bottom_gap = 180.0 - (vertical.track.y + vertical.track.h);
        let right_gap = 240.0 - (vertical.track.x + vertical.track.w);
        assert!(
            right_gap >= 5.0,
            "track should keep right breathing room: {:?}",
            vertical.track
        );
        assert!(
            (bottom_gap - right_gap).abs() <= 1.0,
            "right and bottom breathing room should match: track={:?} right_gap={right_gap} bottom_gap={bottom_gap}",
            vertical.track
        );
    }

    #[test]
    fn titled_modal_scrollbar_track_starts_below_header_band() {
        let mut modal = node("modal", WidgetKind::Modal);
        modal.props.text = Some("Scrollable modal".to_string());
        modal.style.visual.border_radius = Some(12.0);
        modal.style.visual.border_width = Some(1.0);
        modal.style.layout.padding = Some(14.0);
        modal.style.parts.parts.insert(
            "scrollbar-track".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(8.0),
                    padding: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        modal.style.parts.parts.insert(
            "scrollbar-thumb".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(6.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "modal".to_string(),
            Rect {
                x: 50.0,
                y: 40.0,
                w: 320.0,
                h: 180.0,
            },
        );
        layout.scroll_max_y.insert("modal".to_string(), 120.0);
        layout.scroll_y.insert("modal".to_string(), 0.0);

        let geometry = panel_scrollbar_geometry(
            &modal,
            &layout,
            &WidgetState::default(),
            &Theme::dark(),
            1.0,
            Rect {
                x: 50.0,
                y: 40.0,
                w: 320.0,
                h: 180.0,
            },
        )
        .expect("scrollbar geometry");
        let vertical = geometry.vertical.expect("vertical scrollbar");
        let title_inset = panel_scrollbar_title_inset(&modal, &layout, &Theme::dark(), 1.0);

        assert!(
            vertical.track.y >= 40.0 + title_inset,
            "modal scrollbar should start below header band: track={:?} title_inset={title_inset}",
            vertical.track
        );
    }

    #[test]
    fn panel_scrollbar_geometry_stays_anchored_when_parent_clips_panel() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.props.text = Some("Controls".to_string());
        panel.style.visual.border_radius = Some(20.0);
        panel.style.visual.border_width = Some(1.0);
        panel.style.layout.padding = Some(18.0);

        let mut full_layout = LayoutResult::default();
        full_layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        full_layout.clips.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        full_layout.paint_clips.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        full_layout.scroll_max_y.insert("panel".to_string(), 120.0);
        full_layout.scroll_y.insert("panel".to_string(), 0.0);

        let mut clipped_layout = LayoutResult::default();
        clipped_layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        clipped_layout.clips.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 70.0,
                w: 100.0,
                h: 50.0,
            },
        );
        clipped_layout
            .scroll_max_y
            .insert("panel".to_string(), 120.0);
        clipped_layout.scroll_y.insert("panel".to_string(), 0.0);
        clipped_layout.paint_clips.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 70.0,
                w: 100.0,
                h: 50.0,
            },
        );

        let state = WidgetState::default();
        let theme = Theme::dark();
        let mut full_out = Vec::new();
        let mut clipped_out = Vec::new();
        emit_rects(
            &panel,
            &full_layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut full_out,
        );
        emit_rects(
            &panel,
            &clipped_layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut clipped_out,
        );

        let full_track = full_out
            .iter()
            .find(|inst| (inst.rect[2] - 4.0).abs() < 0.01)
            .expect("full scrollbar track");
        let clipped_track = clipped_out
            .iter()
            .find(|inst| (inst.rect[2] - 4.0).abs() < 0.01)
            .expect("clipped scrollbar track");

        assert_eq!(clipped_track.rect, full_track.rect);
        assert!(
            clipped_track.clip[1] > full_track.clip[1],
            "paint clip should hide offscreen scrollbar instead of changing geometry"
        );
    }

    #[test]
    fn panel_scrollbar_suppresses_tiny_rounding_overflow() {
        let panel = node("panel", WidgetKind::Panel);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        layout.scroll_max_y.insert("panel".to_string(), 1.5);

        let geometry = panel_scrollbar_geometry(
            &panel,
            &layout,
            &WidgetState::default(),
            &Theme::dark(),
            1.0,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );

        assert!(
            geometry.is_none(),
            "tiny rounding overflow should not flash a visible scrollbar"
        );
    }

    #[test]
    fn panel_scrollbar_suppresses_unusable_small_tracks() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.parts.parts.insert(
            "scrollbar-track".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(8.0),
                    padding: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        panel.style.parts.parts.insert(
            "scrollbar-thumb".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(6.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 50.0,
            },
        );
        layout.scroll_max_y.insert("panel".to_string(), 32.0);

        let geometry = panel_scrollbar_geometry(
            &panel,
            &layout,
            &WidgetState::default(),
            &Theme::dark(),
            1.0,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 50.0,
            },
        );

        assert!(
            geometry.is_none(),
            "small panels should not draw oversized scrollbar tracks"
        );
    }

    #[test]
    fn panel_horizontal_scrollbar_stays_inside_rounded_panel_surface() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.border_radius = Some(20.0);
        panel.style.visual.border_width = Some(1.0);
        panel.style.layout.padding = Some(18.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 80.0,
            },
        );
        layout.scroll_max_x.insert("panel".to_string(), 180.0);
        layout.scroll_x.insert("panel".to_string(), 45.0);
        let state = WidgetState::default();
        let theme = Theme::dark();
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let scrollbar: Vec<_> = out
            .iter()
            .filter(|inst| (inst.rect[3] - 4.0).abs() < 0.01 && inst.rect[2] > 20.0)
            .collect();
        assert_eq!(
            scrollbar.len(),
            2,
            "track and horizontal thumb should be emitted"
        );
        let track = scrollbar[0];
        let thumb = scrollbar[1];
        assert!(
            track.rect[1] + track.rect[3] <= 71.0,
            "horizontal scrollbar should be inset from the rounded bottom edge: {:?}",
            track.rect
        );
        let left_gap = track.rect[0];
        let right_gap = 140.0 - (track.rect[0] + track.rect[2]);
        assert!(
            left_gap >= 11.0,
            "horizontal scrollbar track should leave enough side breathing room: {:?}",
            track.rect
        );
        assert!(
            (left_gap - right_gap).abs() < 0.01,
            "horizontal scrollbar track should be centered on the panel surface: left_gap={left_gap} right_gap={right_gap}"
        );
        assert!(thumb.rect[0] >= track.rect[0]);
        assert!(thumb.rect[0] + thumb.rect[2] <= track.rect[0] + track.rect[2]);
    }

    #[test]
    fn panel_scrollbars_avoid_bottom_right_corner_overlap() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.border_radius = Some(20.0);
        panel.style.visual.border_width = Some(1.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 100.0,
            },
        );
        layout.scroll_max_x.insert("panel".to_string(), 120.0);
        layout.scroll_x.insert("panel".to_string(), 0.0);
        layout.scroll_max_y.insert("panel".to_string(), 120.0);
        layout.scroll_y.insert("panel".to_string(), 0.0);
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let vertical: Vec<_> = out
            .iter()
            .filter(|inst| (inst.rect[2] - 4.0).abs() < 0.01 && inst.rect[3] > 20.0)
            .collect();
        let horizontal: Vec<_> = out
            .iter()
            .filter(|inst| (inst.rect[3] - 4.0).abs() < 0.01 && inst.rect[2] > 20.0)
            .collect();
        assert_eq!(
            vertical.len(),
            2,
            "vertical track and thumb should be emitted"
        );
        assert_eq!(
            horizontal.len(),
            2,
            "horizontal track and thumb should be emitted"
        );
        let vertical_track = vertical[0];
        let horizontal_track = horizontal[0];
        assert!(vertical_track.rect[1] + vertical_track.rect[3] < horizontal_track.rect[1]);
        assert!(horizontal_track.rect[0] + horizontal_track.rect[2] < vertical_track.rect[0]);
    }

    #[test]
    fn panel_scrollbar_uses_scrollbar_part_styles() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.border_radius = Some(20.0);
        panel.style.parts.parts.insert(
            "scrollbar-track".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(6.0),
                    padding: Some(14.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(ColorRef::Rgba([0.10, 0.20, 0.30, 0.40])),
                    border_radius: Some(99.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        panel.style.parts.parts.insert(
            "scrollbar-thumb".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(8.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(ColorRef::Rgba([0.50, 0.60, 0.70, 0.80])),
                    border_radius: Some(99.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 120.0,
            },
        );
        layout.scroll_max_y.insert("panel".to_string(), 120.0);
        layout.scroll_y.insert("panel".to_string(), 0.0);
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let track = out
            .iter()
            .find(|inst| inst.color == [0.10, 0.20, 0.30, 0.40])
            .expect("styled scrollbar track");
        let thumb = out
            .iter()
            .find(|inst| inst.color == [0.50, 0.60, 0.70, 0.80])
            .expect("styled scrollbar thumb");

        assert_eq!(track.rect, [79.0, 14.0, 6.0, 92.0]);
        assert_eq!(thumb.rect, [78.0, 14.0, 8.0, 46.0]);
        assert_eq!(track.radii, [99.0; 4]);
        assert_eq!(thumb.radii, [99.0; 4]);
    }

    #[test]
    fn modal_scrim_uses_scrim_part_style() {
        let mut modal = node("modal", WidgetKind::Modal);
        modal.props.open = Some(true);
        modal.style.parts.parts.insert(
            "scrim".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(ColorRef::Rgba([0.10, 0.20, 0.30, 0.40])),
                    opacity: Some(0.5),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 300.0,
                h: 200.0,
            },
        );
        layout.rects.insert(
            "modal".to_string(),
            Rect {
                x: 50.0,
                y: 40.0,
                w: 120.0,
                h: 80.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &modal,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        assert!(
            has_rect(&out, [0.10, 0.20, 0.30, 0.20], [0.0, 0.0, 300.0, 200.0]),
            "modal scrim should use styled scrim background and opacity"
        );
    }

    #[test]
    fn titled_modal_header_band_uses_surface_radii() {
        let mut modal = node("modal", WidgetKind::Modal);
        modal.props.open = Some(true);
        modal.props.text = Some("Modal title".to_string());
        modal.style.visual.background = Some(ColorRef::Rgba([0.10, 0.12, 0.16, 1.0]));
        modal.style.visual.accent = Some(ColorRef::Rgba([0.90, 0.20, 0.10, 1.0]));
        modal.style.visual.border_color = Some(ColorRef::Rgba([0.20, 0.22, 0.28, 1.0]));
        modal.style.visual.border_width = Some(2.0);
        modal.style.visual.border_radius = Some(14.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 320.0,
                h: 220.0,
            },
        );
        layout.rects.insert(
            "modal".to_string(),
            Rect {
                x: 50.0,
                y: 40.0,
                w: 180.0,
                h: 110.0,
            },
        );

        let theme = Theme::dark();
        let mut out = Vec::new();
        emit_rects(
            &modal,
            &layout,
            &theme,
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let header_color = mix([0.10, 0.12, 0.16, 1.0], [0.90, 0.20, 0.10, 1.0], 0.16);
        let header = out
            .iter()
            .find(|inst| inst.color == header_color)
            .expect("modal header band");
        let border = out
            .iter()
            .find(|inst| {
                inst.color == [0.20, 0.22, 0.28, 1.0] && inst.rect == [50.0, 40.0, 180.0, 110.0]
            })
            .expect("modal underpainted border shape");
        let title_band_h = titled_container_geometry(&modal, &layout, 1.0, &theme)
            .expect("titled modal geometry")
            .title_band
            .h;
        let fill = out
            .iter()
            .find(|inst| {
                inst.color == [0.10, 0.12, 0.16, 1.0]
                    && inst.rect == [52.0, 42.0 + title_band_h, 176.0, 106.0 - title_band_h]
            })
            .expect("modal body fill");

        assert_eq!(border.radii, [14.0; 4]);
        assert_eq!(fill.radii, [0.0, 0.0, 12.0, 12.0]);
        assert_eq!(header.rect, [52.0, 42.0, 176.0, 106.0]);
        assert_eq!(header.radii, [12.0; 4]);
        assert_eq!(header.clip, [0.0, 0.0, 176.0, title_band_h]);
    }

    #[test]
    fn sidebar_and_modal_consume_shared_titled_surface_parts() {
        for (kind, id, header_color, body_color) in [
            (
                WidgetKind::Sidebar,
                "sidebar",
                [0.81, 0.11, 0.21, 1.0],
                [0.11, 0.21, 0.81, 1.0],
            ),
            (
                WidgetKind::Modal,
                "modal",
                [0.82, 0.12, 0.22, 1.0],
                [0.12, 0.22, 0.82, 1.0],
            ),
        ] {
            let mut container = node(id, kind);
            container.props.text = Some("Shared title".to_string());
            if kind == WidgetKind::Modal {
                container.props.open = Some(true);
            }
            for (part, color) in [("header", header_color), ("body", body_color)] {
                let mut style = PartStyle::default();
                style.visual.background = Some(ColorRef::Rgba(color));
                container.style.parts.parts.insert(part.to_string(), style);
            }
            let mut layout = LayoutResult::default();
            layout.rects.insert(
                "window".to_string(),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 360.0,
                    h: 240.0,
                },
            );
            layout.rects.insert(
                id.to_string(),
                Rect {
                    x: 40.0,
                    y: 30.0,
                    w: 220.0,
                    h: 150.0,
                },
            );
            let mut out = Vec::new();

            emit_rects(
                &container,
                &layout,
                &Theme::dark(),
                1.0,
                &WidgetState::default(),
                &HashMap::new(),
                &mut out,
            );

            assert!(
                out.iter().any(|instance| instance.color == header_color),
                "{kind:?} did not paint ::header"
            );
            assert!(
                out.iter().any(|instance| instance.color == body_color),
                "{kind:?} did not paint ::body"
            );
        }
    }

    #[test]
    fn dataframe_table_border_uses_rounded_ring() {
        let mut table = node("table", WidgetKind::DataFrameTable);
        table.style.visual.background = Some(ColorRef::Rgba([0.02, 0.03, 0.04, 1.0]));
        table.style.visual.border_color = Some(ColorRef::Rgba([0.10, 0.20, 0.80, 1.0]));
        table.style.visual.border_width = Some(2.0);
        table.style.visual.border_radius = Some(12.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "table".to_string(),
            Rect {
                x: 20.0,
                y: 30.0,
                w: 240.0,
                h: 160.0,
            },
        );

        let mut out = Vec::new();
        emit_rects(
            &table,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let border = out
            .iter()
            .find(|inst| inst.color == [0.10, 0.20, 0.80, 1.0])
            .expect("table border ring");

        assert_eq!(border.rect, [20.0, 30.0, 240.0, 160.0]);
        assert_eq!(border.radii, [12.0; 4]);
        assert_eq!(border.params[2], 3.0);
        assert_eq!(border.paint[3], 2.0);
    }

    #[test]
    fn open_modal_overlay_paints_after_document_content() {
        let mut modal = node("modal", WidgetKind::Modal);
        modal.props.open = Some(true);
        modal.style.visual.background = Some(ColorRef::Rgba([0.0, 0.8, 0.2, 1.0]));
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background = Some(ColorRef::Rgba([0.8, 0.0, 0.0, 1.0]));
        let mut root = node("window", WidgetKind::Window);
        root.children = vec![modal, panel];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 300.0,
                h: 220.0,
            },
        );
        layout.rects.insert(
            "modal".to_string(),
            Rect {
                x: 75.0,
                y: 60.0,
                w: 150.0,
                h: 100.0,
            },
        );
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 40.0,
                y: 40.0,
                w: 220.0,
                h: 140.0,
            },
        );

        let mut out = Vec::new();
        let theme = Theme::dark();
        let state = WidgetState::default();
        let carets = HashMap::new();
        let mut icon_geometry_cache = IconGeometryCache::default();
        emit_rects_inner(
            &root,
            &layout,
            &theme,
            1.0,
            &state,
            &carets,
            true,
            RenderContext::default(),
            &mut out,
            None,
            &mut icon_geometry_cache,
        );
        emit_modal_overlays(
            &root,
            &layout,
            &theme,
            1.0,
            &state,
            &carets,
            &mut out,
            &mut icon_geometry_cache,
        );

        let panel_index = out
            .iter()
            .position(|inst| inst.color == [0.8, 0.0, 0.0, 1.0])
            .expect("panel surface");
        let modal_index = out
            .iter()
            .rposition(|inst| inst.color == [0.0, 0.8, 0.2, 1.0])
            .expect("modal surface");

        assert!(
            modal_index > panel_index,
            "open modal should paint after normal content: panel={panel_index} modal={modal_index}"
        );
    }

    #[test]
    fn explicit_layout_scroll_container_emits_scrollbar_indicator() {
        let mut row = node("row", WidgetKind::HLayout);
        row.style.layout.overflow_x = Some(OverflowStyle::Auto);
        row.style.layout.overflow_y = Some(OverflowStyle::Hidden);
        row.style.parts.parts.insert(
            "scrollbar-track".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(5.0),
                    padding: Some(10.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(ColorRef::Rgba([0.12, 0.22, 0.32, 0.42])),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        row.style.parts.parts.insert(
            "scrollbar-thumb".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(7.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(ColorRef::Rgba([0.52, 0.62, 0.72, 0.82])),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "row".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 52.0,
            },
        );
        layout.scroll_max_x.insert("row".to_string(), 180.0);
        layout.scroll_x.insert("row".to_string(), 40.0);
        let mut out = Vec::new();

        emit_rects(
            &row,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let track = out
            .iter()
            .find(|inst| inst.color == [0.12, 0.22, 0.32, 0.42])
            .expect("styled HLayout scrollbar track");
        let thumb = out
            .iter()
            .find(|inst| inst.color == [0.52, 0.62, 0.72, 0.82])
            .expect("styled HLayout scrollbar thumb");
        assert_eq!(track.rect[3], 5.0);
        assert_eq!(thumb.rect[3], 7.0);
        assert!(thumb.rect[0] > track.rect[0]);
        assert!(thumb.rect[0] + thumb.rect[2] <= track.rect[0] + track.rect[2]);
    }

    #[test]
    fn generic_scrollbar_renderer_consumes_shared_part_colors() {
        let mut row = node("row", WidgetKind::HLayout);
        row.style.layout.overflow_x = Some(OverflowStyle::Auto);
        row.style.layout.overflow_y = Some(OverflowStyle::Hidden);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            row.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 52.0,
            },
        );
        layout.scroll_max_x.insert(row.id.clone(), 180.0);
        layout.scroll_x.insert(row.id.clone(), 40.0);
        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut out = Vec::new();

        emit_rects(
            &row,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        for part in ["scrollbar-track", "scrollbar-thumb"] {
            let color = widget_part_paint_fallback(&row, part, &theme, &state)
                .background
                .expect("generic scrollbar part fallback");
            assert!(
                out.iter().any(|instance| instance.color == color),
                "missing cataloged HLayout::{part} color"
            );
        }
    }

    #[test]
    fn radial_gradient_background_emits_gradient_rect_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::RadialGradient(RadialGradient {
                repeating: false,
                center: [0.5, 0.5],
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]),
                        position: None,
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]),
                        position: None,
                    },
                ],
            }));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let fill = out
            .iter()
            .find(|inst| inst.paint[0] == 2.0)
            .expect("radial gradient fill instance");
        assert_eq!(fill.color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(fill.color2, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(fill.paint[1], 0.5);
        assert_eq!(fill.paint[2], 0.5);
        assert_eq!(fill.paint[3], 2.0);
    }

    #[test]
    fn semantic_pseudo_visuals_resolve_from_widget_state() {
        let mut dropdown = node("mode", WidgetKind::Dropdown);
        dropdown.style.open.border_color = Some(rgba(0.2, 0.4, 0.6));
        let mut state = WidgetState {
            open_dropdown: Some("mode".to_string()),
            ..Default::default()
        };

        let theme = Theme::dark();
        let visual = visual_for(&dropdown, &state, &theme);
        assert_eq!(visual.border_color, Some(rgba(0.2, 0.4, 0.6)));

        let mut tab = node("tab-a", WidgetKind::Tab);
        tab.style.selected.background = Some(rgba(0.3, 0.5, 0.7));
        state.open_dropdown = None;
        state
            .tab_parent
            .insert("tab-a".to_string(), "tabs".to_string());
        state
            .tab_values
            .insert("tab-a".to_string(), "a".to_string());
        state
            .active_tabs
            .insert("tabs".to_string(), "a".to_string());

        let theme = Theme::dark();
        let visual = visual_for(&tab, &state, &theme);
        assert_eq!(visual.background, Some(rgba(0.3, 0.5, 0.7)));
    }

    #[test]
    fn drag_drop_overlay_emits_cursor_ring_and_follow_chip() {
        let theme = Theme::dark();
        let state = WidgetState {
            drag_source: Some("source".to_string()),
            drag_kind: Some("asset".to_string()),
            drag_pos: Some([100.0, 80.0]),
            ..Default::default()
        };
        let mut out = Vec::new();

        emit_drag_drop_overlay(&state, &theme, 1.0, 400.0, 300.0, &mut out);

        assert!(out.len() >= 7, "drag ghost should emit a visible overlay");
        assert!(
            out.iter().any(|inst| inst.rect == [94.0, 74.0, 12.0, 12.0]),
            "drag ghost should include a cursor-local ring"
        );
        assert!(
            out.iter()
                .any(|inst| inst.rect == [114.0, 94.0, 54.0, 28.0]),
            "drag ghost should include a chip offset from the pointer"
        );

        let valid_target = WidgetState {
            drag_source: Some("source".to_string()),
            drag_hover_target: Some("target".to_string()),
            drag_pos: Some([100.0, 80.0]),
            ..Default::default()
        };
        out.clear();
        emit_drag_drop_overlay(&valid_target, &theme, 1.0, 400.0, 300.0, &mut out);
        assert!(
            out.iter()
                .any(|inst| inst.color == with_alpha(theme.success, 0.96)),
            "compatible target state should switch the drag ghost accent to success"
        );
    }

    #[test]
    fn hover_transition_progress_interpolates_visual_fields() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        button.style.visual.border_width = Some(1.0);
        button.style.hover.background = Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]));
        button.style.hover.border_width = Some(3.0);
        let mut state = WidgetState::default();
        state.hover_t.insert("run".to_string(), 0.5);

        let theme = Theme::dark();
        let visual = visual_for(&button, &state, &theme);

        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.5, 0.5, 0.5, 1.0]))
        );
        assert_eq!(visual.border_width, Some(2.0));
    }

    #[test]
    fn transition_property_limits_hover_interpolation() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        button.style.visual.border_width = Some(1.0);
        button.style.hover.background = Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]));
        button.style.hover.border_width = Some(3.0);
        button.style.transition.properties = Some(vec![TransitionProperty::Background]);

        let theme = Theme::dark();
        let mut entering = WidgetState {
            hovered: Some("run".to_string()),
            ..Default::default()
        };
        entering.hover_t.insert("run".to_string(), 0.5);
        let visual = visual_for(&button, &entering, &theme);
        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.5, 0.5, 0.5, 1.0]))
        );
        assert_eq!(visual.border_width, Some(3.0));

        let mut leaving = WidgetState::default();
        leaving.hover_t.insert("run".to_string(), 0.5);
        let visual = visual_for(&button, &leaving, &theme);
        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.5, 0.5, 0.5, 1.0]))
        );
        assert_eq!(visual.border_width, Some(1.0));
    }

    #[test]
    fn interactive_state_visuals_do_not_mutate_computed_or_authored_layers() {
        let mut button = node("run", WidgetKind::Button);
        let base = ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]);
        let hover = ColorRef::Rgba([0.2, 0.4, 0.6, 1.0]);
        let active = ColorRef::Rgba([0.3, 0.5, 0.7, 1.0]);
        let focus = ColorRef::Rgba([0.4, 0.6, 0.8, 1.0]);
        let disabled = ColorRef::Rgba([0.5, 0.5, 0.5, 1.0]);
        button.style.visual.background = Some(base.clone());
        button.style.hover.background = Some(hover.clone());
        button.style.active.background = Some(active.clone());
        button.style.focus.background = Some(focus.clone());
        button.style.disabled.background = Some(disabled.clone());
        button.inline_style.visual.background = Some(base.clone());
        button.style_json.insert(
            "background".to_string(),
            serde_json::json!([0.1, 0.2, 0.3, 1.0]),
        );
        let authored_json = button.style_json.clone();
        let theme = Theme::dark();

        let states = [
            WidgetState::default(),
            WidgetState {
                hovered: Some("run".to_string()),
                ..Default::default()
            },
            WidgetState {
                pressed: Some("run".to_string()),
                ..Default::default()
            },
            WidgetState {
                focused: Some("run".to_string()),
                ..Default::default()
            },
            WidgetState {
                disabled: std::collections::HashSet::from(["run".to_string()]),
                ..Default::default()
            },
            WidgetState::default(),
        ];
        let expected = [&base, &hover, &active, &focus, &disabled, &base];

        for (state, expected_background) in states.iter().zip(expected) {
            let visual = visual_for(&button, state, &theme);
            assert_eq!(visual.background.as_ref(), Some(expected_background));
            assert_eq!(button.style.visual.background.as_ref(), Some(&base));
            assert_eq!(button.style.hover.background.as_ref(), Some(&hover));
            assert_eq!(button.style.active.background.as_ref(), Some(&active));
            assert_eq!(button.style.focus.background.as_ref(), Some(&focus));
            assert_eq!(button.style.disabled.background.as_ref(), Some(&disabled));
            assert_eq!(button.inline_style.visual.background.as_ref(), Some(&base));
            assert_eq!(button.style_json, authored_json);
        }
    }

    #[test]
    fn checked_transition_progress_interpolates_visual_fields() {
        let mut checkbox = node("enabled", WidgetKind::Checkbox);
        checkbox.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        checkbox.style.visual.border_width = Some(1.0);
        checkbox.style.checked.background = Some(ColorRef::Rgba([0.2, 0.8, 0.4, 1.0]));
        checkbox.style.checked.border_width = Some(5.0);
        let mut state = WidgetState::default();
        state.checked.insert("enabled".to_string(), true);
        state.checked_t.insert("enabled".to_string(), 0.5);

        let theme = Theme::dark();
        let visual = visual_for(&checkbox, &state, &theme);

        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.1, 0.4, 0.2, 1.0]))
        );
        assert_eq!(visual.border_width, Some(3.0));
    }

    #[test]
    fn checked_control_renderers_consume_shared_part_colors() {
        let theme = Theme::dark();
        let mut checkbox = node("enabled", WidgetKind::Checkbox);
        checkbox.props.checked = Some(true);
        let mut checkbox_layout = LayoutResult::default();
        checkbox_layout.rects.insert(
            checkbox.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 32.0,
            },
        );
        let checkbox_state = WidgetState::from_tree(&checkbox);
        let mut checkbox_out = Vec::new();
        emit_rects(
            &checkbox,
            &checkbox_layout,
            &theme,
            1.0,
            &checkbox_state,
            &HashMap::new(),
            &mut checkbox_out,
        );
        let checkbox_box = widget_part_paint_fallback(&checkbox, "box", &theme, &checkbox_state)
            .background
            .expect("checked checkbox box fallback");
        let checkbox_indicator =
            widget_part_paint_fallback(&checkbox, "indicator", &theme, &checkbox_state)
                .background
                .expect("checked checkbox indicator fallback");
        assert!(checkbox_out
            .iter()
            .any(|instance| instance.color == checkbox_box));
        assert!(checkbox_out
            .iter()
            .any(|instance| instance.color == checkbox_indicator));

        let mut toggle = node("wifi", WidgetKind::ToggleSwitch);
        toggle.props.checked = Some(true);
        let mut toggle_layout = LayoutResult::default();
        toggle_layout.rects.insert(
            toggle.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 32.0,
            },
        );
        let mut toggle_state = WidgetState::from_tree(&toggle);
        toggle_state.checked_t.insert(toggle.id.clone(), 1.0);
        let mut toggle_out = Vec::new();
        emit_rects(
            &toggle,
            &toggle_layout,
            &theme,
            1.0,
            &toggle_state,
            &HashMap::new(),
            &mut toggle_out,
        );
        let toggle_track = widget_part_paint_fallback(&toggle, "track", &theme, &toggle_state)
            .background
            .expect("checked toggle track fallback");
        let toggle_thumb = widget_part_paint_fallback(&toggle, "thumb", &theme, &toggle_state)
            .background
            .expect("toggle thumb fallback");
        assert!(toggle_out
            .iter()
            .any(|instance| instance.color == toggle_track));
        assert!(toggle_out
            .iter()
            .any(|instance| instance.color == toggle_thumb));
    }

    #[test]
    fn radio_button_renderer_consumes_shared_indicator_and_selected_dot_paint() {
        let radio = node("choice", WidgetKind::RadioButton);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            radio.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 32.0,
            },
        );
        let theme = Theme::dark();
        let mut state = WidgetState::from_tree(&radio);

        for selected in [false, true] {
            state.selectable_selected.insert(radio.id.clone(), selected);
            let mut out = Vec::new();
            emit_rects(
                &radio,
                &layout,
                &theme,
                1.0,
                &state,
                &HashMap::new(),
                &mut out,
            );

            let indicator = native_widget_part_paint_fallback(
                WidgetKind::RadioButton,
                "indicator",
                &theme,
                PaintInteraction::Resting,
                selected,
            );
            for color in [
                indicator.background.expect("radio indicator fill fallback"),
                indicator
                    .border_color
                    .expect("radio indicator border fallback"),
            ] {
                assert!(out.iter().any(|instance| {
                    instance.color == color && instance.rect[2] <= 14.0 && instance.rect[3] <= 14.0
                }));
            }

            let dot = native_widget_part_paint_fallback(
                WidgetKind::RadioButton,
                "dot",
                &theme,
                PaintInteraction::Resting,
                selected,
            );
            if selected {
                let dot = dot.background.expect("selected radio dot fallback");
                assert!(out.iter().any(|instance| {
                    instance.color == dot && instance.rect[2] <= 6.0 && instance.rect[3] <= 6.0
                }));
            } else {
                assert_eq!(dot, NativePaintFallback::default());
            }
        }
    }

    #[test]
    fn selectable_renderer_consumes_shared_row_and_selected_indicator_paint() {
        let selectable = node("choice", WidgetKind::Selectable);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            selectable.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 32.0,
            },
        );
        let theme = Theme::dark();

        let mut hovered = WidgetState::from_tree(&selectable);
        hovered.hovered = Some(selectable.id.clone());
        let mut hovered_out = Vec::new();
        emit_rects(
            &selectable,
            &layout,
            &theme,
            1.0,
            &hovered,
            &HashMap::new(),
            &mut hovered_out,
        );
        let hovered_row = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Selectable,
            "row",
            &theme,
            PaintInteraction::Hovered,
            false,
            false,
        )
        .background
        .expect("hovered selectable row fallback");
        assert!(hovered_out
            .iter()
            .any(|instance| instance.color == hovered_row));

        let mut selected = WidgetState::from_tree(&selectable);
        selected
            .selectable_selected
            .insert(selectable.id.clone(), true);
        let mut selected_out = Vec::new();
        emit_rects(
            &selectable,
            &layout,
            &theme,
            1.0,
            &selected,
            &HashMap::new(),
            &mut selected_out,
        );
        for fallback in [
            native_widget_part_paint_fallback_with_selection(
                WidgetKind::Selectable,
                "row",
                &theme,
                PaintInteraction::Resting,
                false,
                true,
            ),
            native_widget_part_paint_fallback_with_selection(
                WidgetKind::Selectable,
                "indicator",
                &theme,
                PaintInteraction::Resting,
                false,
                true,
            ),
        ] {
            let color = fallback.background.expect("selected selectable fallback");
            assert!(selected_out.iter().any(|instance| instance.color == color));
        }
    }

    #[test]
    fn unlabeled_checkbox_row_and_focus_stay_tight_to_box() {
        let mut checkbox = node("enabled", WidgetKind::Checkbox);
        checkbox.props.text = Some(String::new());
        checkbox.style.parts.parts.insert(
            "row".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.20, 0.30, 0.40)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "enabled".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 90.0,
                h: 32.0,
            },
        );
        let theme = Theme::dark();
        let state = WidgetState {
            focused: Some("enabled".to_string()),
            ..Default::default()
        };
        let mut out = Vec::new();

        emit_rects(
            &checkbox,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let row = out
            .iter()
            .find(|inst| inst.color == [0.20, 0.30, 0.40, 1.0])
            .expect("checkbox row fill should be emitted");
        assert_eq!(row.rect, [0.0, 0.0, 25.0, 32.0]);

        let focus = out
            .iter()
            .find(|inst| inst.color == with_alpha(theme.focus, 0.60) && inst.params[2] == 3.0)
            .expect("checkbox focus ring should be emitted");
        assert_eq!(focus.rect, [-2.0, -2.0, 29.0, 36.0]);
    }

    #[test]
    fn toggle_switch_checked_state_places_thumb_on_right() {
        let mut toggle = node("wifi", WidgetKind::ToggleSwitch);
        toggle.props.checked = Some(true);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "wifi".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 32.0,
            },
        );
        let mut state = WidgetState::from_tree(&toggle);
        state.checked_t.insert("wifi".to_string(), 1.0);
        let mut out = Vec::new();

        emit_rects(
            &toggle,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        assert!(
            out.iter().any(|inst| inst.rect[0] >= 22.0
                && inst.rect[0] <= 24.0
                && inst.rect[2] >= 12.0
                && inst.rect[2] <= 14.0),
            "checked ToggleSwitch should draw its thumb near the right side of the track; rects={:?}",
            out.iter().map(|inst| inst.rect).collect::<Vec<_>>()
        );
    }

    #[test]
    fn active_transition_progress_interpolates_visual_fields() {
        let mut button = node("submit", WidgetKind::Button);
        button.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        button.style.visual.border_width = Some(1.0);
        button.style.active.background = Some(ColorRef::Rgba([0.8, 0.2, 0.1, 1.0]));
        button.style.active.border_width = Some(5.0);
        let mut state = WidgetState {
            pressed: Some("submit".to_string()),
            ..Default::default()
        };
        state.active_t.insert("submit".to_string(), 0.5);

        let theme = Theme::dark();
        let visual = visual_for(&button, &state, &theme);

        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.4, 0.1, 0.05, 1.0]))
        );
        assert_eq!(visual.border_width, Some(3.0));
    }

    #[test]
    fn focus_transition_progress_interpolates_visual_fields() {
        let mut input = node("amount", WidgetKind::TextInput);
        input.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        input.style.visual.border_width = Some(1.0);
        input.style.focus.background = Some(ColorRef::Rgba([0.1, 0.4, 0.9, 1.0]));
        input.style.focus.border_width = Some(3.0);
        let mut state = WidgetState {
            focused: Some("amount".to_string()),
            ..Default::default()
        };
        state.focus_t.insert("amount".to_string(), 0.5);

        let theme = Theme::dark();
        let visual = visual_for(&input, &state, &theme);

        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.05, 0.2, 0.45, 1.0]))
        );
        assert_eq!(visual.border_width, Some(2.0));
    }

    #[test]
    fn outline_transition_properties_interpolate_visual_fields() {
        let mut badge = node("status", WidgetKind::Badge);
        badge.style.visual.outline_color = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        badge.style.visual.outline_width = Some(1.0);
        badge.style.visual.outline_offset = Some(2.0);
        badge.style.hover.outline_color = Some(ColorRef::Rgba([1.0, 0.5, 0.0, 1.0]));
        badge.style.hover.outline_width = Some(5.0);
        badge.style.hover.outline_offset = Some(10.0);
        badge.style.transition.properties = Some(vec![
            TransitionProperty::OutlineColor,
            TransitionProperty::OutlineWidth,
        ]);

        let theme = Theme::dark();
        let mut state = WidgetState {
            hovered: Some("status".to_string()),
            ..Default::default()
        };
        state.hover_t.insert("status".to_string(), 0.5);
        let visual = visual_for(&badge, &state, &theme);

        assert_eq!(
            visual.outline_color,
            Some(ColorRef::Rgba([0.5, 0.25, 0.0, 1.0]))
        );
        assert_eq!(visual.outline_width, Some(3.0));
        assert_eq!(visual.outline_offset, Some(10.0));
    }

    #[test]
    fn outline_transition_shorthand_interpolates_offset_too() {
        let mut badge = node("status", WidgetKind::Badge);
        badge.style.visual.outline_width = Some(1.0);
        badge.style.visual.outline_offset = Some(2.0);
        badge.style.hover.outline_width = Some(5.0);
        badge.style.hover.outline_offset = Some(10.0);
        badge.style.transition.properties = Some(vec![TransitionProperty::Outline]);

        let theme = Theme::dark();
        let mut state = WidgetState {
            hovered: Some("status".to_string()),
            ..Default::default()
        };
        state.hover_t.insert("status".to_string(), 0.5);
        let visual = visual_for(&badge, &state, &theme);

        assert_eq!(visual.outline_width, Some(3.0));
        assert_eq!(visual.outline_offset, Some(6.0));
    }

    #[test]
    fn open_transition_progress_interpolates_visual_fields() {
        let mut dropdown = node("mode", WidgetKind::Dropdown);
        dropdown.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        dropdown.style.visual.border_width = Some(1.0);
        dropdown.style.open.background = Some(ColorRef::Rgba([0.0, 0.5, 1.0, 1.0]));
        dropdown.style.open.border_width = Some(3.0);
        let mut state = WidgetState {
            open_dropdown: Some("mode".to_string()),
            ..Default::default()
        };
        state.open_t.insert("mode".to_string(), 0.5);

        let theme = Theme::dark();
        let visual = visual_for(&dropdown, &state, &theme);

        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.0, 0.25, 0.5, 1.0]))
        );
        assert_eq!(visual.border_width, Some(2.0));
    }

    #[test]
    fn selected_transition_progress_interpolates_visual_fields() {
        let mut tab = node("tab-a", WidgetKind::Tab);
        tab.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        tab.style.visual.border_width = Some(1.0);
        tab.style.selected.background = Some(ColorRef::Rgba([0.6, 0.2, 0.0, 1.0]));
        tab.style.selected.border_width = Some(3.0);
        let mut state = WidgetState::default();
        state
            .tab_parent
            .insert("tab-a".to_string(), "tabs".to_string());
        state
            .tab_values
            .insert("tab-a".to_string(), "a".to_string());
        state
            .active_tabs
            .insert("tabs".to_string(), "a".to_string());
        state.selected_t.insert("tab-a".to_string(), 0.5);

        let theme = Theme::dark();
        let visual = visual_for(&tab, &state, &theme);

        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.3, 0.1, 0.0, 1.0]))
        );
        assert_eq!(visual.border_width, Some(2.0));
    }

    #[test]
    fn expanded_transition_progress_interpolates_visual_fields() {
        let mut collapsible = node("advanced", WidgetKind::Collapsible);
        collapsible.style.visual.background = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 1.0]));
        collapsible.style.visual.border_width = Some(1.0);
        collapsible.style.expanded.background = Some(ColorRef::Rgba([0.0, 0.5, 1.0, 1.0]));
        collapsible.style.expanded.border_width = Some(3.0);
        collapsible.style.collapsed.background = Some(ColorRef::Rgba([0.2, 0.0, 0.0, 1.0]));
        collapsible.style.collapsed.border_width = Some(5.0);
        let mut state = WidgetState::default();
        state.expanded.insert("advanced".to_string(), true);
        state.expanded_t.insert("advanced".to_string(), 0.5);

        let theme = Theme::dark();
        let visual = visual_for(&collapsible, &state, &theme);

        assert_eq!(
            visual.background,
            Some(ColorRef::Rgba([0.1, 0.25, 0.5, 1.0]))
        );
        assert_eq!(visual.border_width, Some(4.0));
    }

    #[test]
    fn transform_style_is_encoded_on_widget_primitives() {
        let mut button = node("run", WidgetKind::Button);
        button.style.visual.transform = Some(TransformStyle {
            translate_x: 3.0,
            translate_y: -2.0,
            scale_x: 1.05,
            scale_y: 0.95,
            rotate_deg: 5.0,
        });

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 30.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &button,
            &layout,
            &Theme::dark(),
            2.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let surface = out.last().expect("button surface primitive");
        assert_eq!(surface.transform, [6.0, -4.0, 1.05, 0.95]);
        assert!((surface.transform2[0] - 5.0_f32.to_radians()).abs() < 0.001);
    }

    #[test]
    fn container_transform_propagates_to_child_widget_primitives() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.transform = Some(TransformStyle {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 2.0,
            scale_y: 2.0,
            rotate_deg: 0.0,
        });
        panel.children.push(node("run", WidgetKind::Button));

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
        );
        layout.rects.insert(
            "run".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 20.0,
                h: 20.0,
            },
        );

        let mut out = Vec::new();
        emit_rects(
            &panel,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let child_surface = out.last().expect("child button primitive");
        assert_eq!(child_surface.transform, [-30.0, -30.0, 2.0, 2.0]);
        assert_eq!(child_surface.transform2[0], 0.0);
    }

    #[test]
    fn relative_position_offsets_widget_primitives() {
        let mut badge = node("badge", WidgetKind::Badge);
        badge.style.layout.position = Some(PositionStyle::Relative);
        badge.style.layout.left = Some(8.0);
        badge.style.layout.top = Some(-6.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "badge".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 80.0,
                h: 24.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &badge,
            &layout,
            &Theme::dark(),
            1.5,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let surface = out.last().expect("badge surface primitive");
        assert_eq!(surface.transform, [12.0, -9.0, 1.0, 1.0]);
    }

    #[test]
    fn z_index_orders_sibling_widget_primitives() {
        let mut back = node("back", WidgetKind::Badge);
        back.style.visual.background = Some(rgba(1.0, 0.0, 0.0));
        back.style.layout.z_index = Some(2);
        let mut front = node("front", WidgetKind::Badge);
        front.style.visual.background = Some(rgba(0.0, 1.0, 0.0));
        front.style.layout.z_index = Some(1);
        let mut parent = node("parent", WidgetKind::VLayout);
        parent.children = vec![back, front];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "parent".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
        );
        layout.rects.insert(
            "back".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 40.0,
                h: 20.0,
            },
        );
        layout.rects.insert(
            "front".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 40.0,
                h: 20.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &parent,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let badge_fills: Vec<_> = out
            .iter()
            .filter(|instance| instance.rect[2] == 40.0 && instance.rect[3] == 20.0)
            .map(|instance| instance.color)
            .collect();
        assert_eq!(badge_fills[0], [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(badge_fills[1], [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn dropdown_chevron_emits_rounded_triangle_mark() {
        let mut dropdown = node("mode", WidgetKind::Dropdown);
        dropdown.style.parts.parts.insert(
            "chevron".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(12.0),
                    ..Default::default()
                },
                text: TextStyle {
                    color: Some(rgba(0.20, 0.30, 0.40)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "mode".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 32.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &dropdown,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let chevron_marks: Vec<_> = out
            .iter()
            .filter(|inst| inst.color == [0.20, 0.30, 0.40, 1.0])
            .collect();
        assert_eq!(chevron_marks.len(), 1);
        assert_eq!(chevron_marks[0].params[3], 1.0);
        assert_eq!(chevron_marks[0].paint[3], 0.0);
        assert!(chevron_marks[0].radii[0] > 0.0);
    }

    #[test]
    fn open_dropdown_chevron_flips_up() {
        let mut dropdown = node("mode", WidgetKind::Dropdown);
        dropdown.style.parts.parts.insert(
            "chevron".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(rgba(0.20, 0.30, 0.40)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "mode".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 32.0,
            },
        );
        let state = WidgetState {
            open_dropdown: Some("mode".to_string()),
            ..Default::default()
        };
        let mut out = Vec::new();

        emit_rects(
            &dropdown,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let chevron_marks: Vec<_> = out
            .iter()
            .filter(|inst| inst.color == [0.20, 0.30, 0.40, 1.0])
            .collect();
        assert_eq!(chevron_marks.len(), 1);
        assert_eq!(chevron_marks[0].params[3], 1.0);
        assert_eq!(chevron_marks[0].paint[3], 1.0);
        assert!(chevron_marks[0].radii[0] > 0.0);
    }

    #[test]
    fn collapsible_indicator_uses_rounded_triangle_mark() {
        let mut collapsible = node("advanced", WidgetKind::Collapsible);
        collapsible.style.parts.parts.insert(
            "indicator".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(rgba(0.20, 0.30, 0.40)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "advanced".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 40.0,
            },
        );
        let mut state = WidgetState::default();
        state.expanded.insert("advanced".to_string(), false);
        let mut out = Vec::new();

        emit_rects(
            &collapsible,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let collapsed_marks: Vec<_> = out
            .iter()
            .filter(|inst| inst.color == [0.20, 0.30, 0.40, 1.0])
            .collect();
        assert_eq!(collapsed_marks.len(), 1);
        assert_eq!(collapsed_marks[0].params[3], 1.0);
        assert_eq!(collapsed_marks[0].paint[3], 0.0);

        state.expanded.insert("advanced".to_string(), true);
        out.clear();
        emit_rects(
            &collapsible,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let expanded_marks: Vec<_> = out
            .iter()
            .filter(|inst| inst.color == [0.20, 0.30, 0.40, 1.0])
            .collect();
        assert_eq!(expanded_marks.len(), 1);
        assert_eq!(expanded_marks[0].params[3], 1.0);
        assert_eq!(expanded_marks[0].paint[3], 1.0);
    }

    #[test]
    fn sorted_table_header_uses_rounded_triangle_mark() {
        let mut table = node("table", WidgetKind::DataFrameTable);
        table.style.parts.parts.insert(
            "header".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(rgba(0.20, 0.30, 0.40)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "table".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 320.0,
                h: 120.0,
            },
        );
        let mut state = WidgetState::default();
        state.tables.insert(
            "table".to_string(),
            crate::events::TableState {
                columns: vec!["alpha".to_string(), "beta".to_string()],
                dtypes: vec!["f64".to_string(), "f64".to_string()],
                rows: 4,
                resource_id: None,
                page_size: 100,
                scroll_row: 0,
                scroll_col: 0,
                selected: None,
                sort: Some((TableSortColumn::Data(1), SortDirection::Asc)),
                row_order: None,
                column_widths: Vec::new(),
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &table,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let marks: Vec<_> = out
            .iter()
            .filter(|inst| inst.color == [0.20, 0.30, 0.40, 1.0])
            .collect();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].params[3], 1.0);
        assert_eq!(marks[0].paint[3], 1.0);
    }

    #[test]
    fn dataframe_table_scrollbar_geometry_tracks_row_and_column_scroll() {
        let mut table = node("table", WidgetKind::DataFrameTable);
        table.style.widget.table_header_height = Some(28.0);
        table.style.widget.table_row_height = Some(24.0);
        table.style.widget.table_index_width = Some(48.0);
        table.style.widget.table_column_width = Some(110.0);
        let mut state = WidgetState::default();
        state.tables.insert(
            "table".to_string(),
            crate::events::TableState {
                columns: (0..8).map(|idx| format!("c{idx}")).collect(),
                dtypes: vec![],
                rows: 40,
                resource_id: None,
                page_size: 100,
                scroll_row: 10,
                scroll_col: 2,
                selected: None,
                sort: None,
                row_order: None,
                column_widths: Vec::new(),
            },
        );

        let geometry = table_scrollbar_geometry(
            &table,
            &state,
            &Theme::dark(),
            1.0,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 280.0,
                h: 150.0,
            },
        )
        .expect("scrollbar geometry");
        let vertical = geometry.vertical.expect("vertical scrollbar");
        let horizontal = geometry.horizontal.expect("horizontal scrollbar");

        assert!(vertical.thumb.y > vertical.track.y);
        assert!(horizontal.thumb.x > horizontal.track.x);

        let theme = Theme::dark();
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            table.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 280.0,
                h: 150.0,
            },
        );
        let mut out = Vec::new();
        emit_rects(
            &table,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );
        for part in ["scrollbar-track", "scrollbar-thumb"] {
            let color = widget_part_paint_fallback(&table, part, &theme, &state)
                .background
                .expect("table scrollbar part fallback");
            assert!(
                out.iter().any(|instance| instance.color == color),
                "missing cataloged DataFrameTable::{part} color"
            );
        }
    }

    #[test]
    fn clipped_collapsible_indicator_keeps_full_widget_position() {
        let mut collapsible = node("advanced", WidgetKind::Collapsible);
        collapsible.style.parts.parts.insert(
            "indicator".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(rgba(0.20, 0.30, 0.40)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "advanced".to_string(),
            Rect {
                x: 0.0,
                y: 30.0,
                w: 180.0,
                h: 40.0,
            },
        );
        layout.clips.insert(
            "advanced".to_string(),
            Rect {
                x: 0.0,
                y: 30.0,
                w: 180.0,
                h: 20.0,
            },
        );
        let mut state = WidgetState::default();
        state.expanded.insert("advanced".to_string(), false);
        let mut out = Vec::new();

        emit_rects(
            &collapsible,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let mark = out
            .iter()
            .find(|inst| inst.color == [0.20, 0.30, 0.40, 1.0])
            .expect("collapsible indicator should be emitted");
        assert_eq!(mark.params[3], 1.0);
        assert!(mark.rect[1] >= 39.5, "indicator rect={:?}", mark.rect);
        assert!(
            mark.clip[3] < mark.rect[3],
            "indicator should be locally clipped"
        );
    }

    #[test]
    fn active_tab_uses_top_only_radii_and_square_accent() {
        let mut tab = node("tab-a", WidgetKind::Tab);
        tab.style.parts.parts.insert(
            "accent".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.11, 0.22, 0.33)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "tab-a".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 36.0,
            },
        );
        let mut state = WidgetState::default();
        state
            .tab_parent
            .insert("tab-a".to_string(), "tabs".to_string());
        state
            .tab_values
            .insert("tab-a".to_string(), "a".to_string());
        state
            .active_tabs
            .insert("tabs".to_string(), "a".to_string());
        let mut out = Vec::new();

        emit_rects(
            &tab,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let accent = out
            .iter()
            .find(|inst| inst.color == [0.11, 0.22, 0.33, 1.0])
            .expect("active tab accent should be emitted");
        assert_eq!(accent.radii, [0.0; 4]);

        let tab_surface = out
            .iter()
            .find(|inst| {
                inst.radii[0] > 0.0
                    && inst.radii[1] > 0.0
                    && inst.radii[2] == 0.0
                    && inst.radii[3] == 0.0
            })
            .unwrap_or_else(|| {
                panic!(
                    "active tab body should be emitted; rects={:?}",
                    out.iter().map(|inst| inst.rect).collect::<Vec<_>>()
                )
            });
        assert!(tab_surface.radii[0] > 0.0);
        assert!(tab_surface.radii[1] > 0.0);
        assert_eq!(tab_surface.radii[2], 0.0);
        assert_eq!(tab_surface.radii[3], 0.0);
    }

    #[test]
    fn tabs_header_surface_is_not_painted_by_default() {
        let mut tabs = node("tabs", WidgetKind::Tabs);
        let tab = node("tab-a", WidgetKind::Tab);
        tabs.children = vec![tab];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "tabs".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 180.0,
            },
        );
        layout.rects.insert(
            "tab-a".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 36.0,
            },
        );
        let mut state = WidgetState::default();
        state
            .tab_parent
            .insert("tab-a".to_string(), "tabs".to_string());
        state
            .tab_values
            .insert("tab-a".to_string(), "a".to_string());
        state
            .active_tabs
            .insert("tabs".to_string(), "a".to_string());
        let mut out = Vec::new();

        emit_rects(
            &tabs,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        assert!(
            !out.iter().any(|inst| inst.rect == [0.0, 0.0, 220.0, 36.0]),
            "unstyled Tabs should not paint a full header box behind tab buttons"
        );
        assert!(
            !out.iter().any(|inst| inst.rect == [0.0, 35.0, 220.0, 1.0]),
            "unstyled Tabs should not paint a header divider line"
        );
    }

    #[test]
    fn explicitly_styled_tabs_header_surface_still_paints() {
        let mut tabs = node("tabs", WidgetKind::Tabs);
        tabs.style.parts.parts.insert(
            "header".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    height: Some(36.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.12, 0.24, 0.36)),
                    border_color: Some(rgba(0.45, 0.55, 0.65)),
                    border_width: Some(2.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "tabs".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 180.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &tabs,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        assert!(out
            .iter()
            .any(|inst| inst.rect == [0.0, 0.0, 220.0, 36.0]
                && inst.color == [0.12, 0.24, 0.36, 1.0]));
        assert!(out
            .iter()
            .any(|inst| inst.rect == [0.0, 34.0, 220.0, 2.0]
                && inst.color == [0.45, 0.55, 0.65, 1.0]));
    }

    #[test]
    fn panel_at_active_tab_body_start_connects_with_square_top_corners() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background = Some(rgba(0.18, 0.28, 0.38));
        panel.style.visual.border_radius = Some(10.0);
        let mut tab = node("tab-a", WidgetKind::Tab);
        tab.children = vec![panel];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "tab-a".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 160.0,
                h: 36.0,
            },
        );
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 0.0,
                y: 36.0,
                w: 160.0,
                h: 120.0,
            },
        );
        let mut state = WidgetState::default();
        state
            .tab_parent
            .insert("tab-a".to_string(), "tabs".to_string());
        state
            .tab_values
            .insert("tab-a".to_string(), "a".to_string());
        state
            .active_tabs
            .insert("tabs".to_string(), "a".to_string());
        let mut out = Vec::new();

        emit_rects(
            &tab,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let panel_surface = out
            .iter()
            .find(|inst| inst.color == [0.18, 0.28, 0.38, 1.0])
            .expect("active tab content panel surface should be emitted");
        assert_eq!(panel_surface.radii[0], 0.0);
        assert_eq!(panel_surface.radii[1], 0.0);
        assert!(panel_surface.radii[2] > 0.0);
        assert!(panel_surface.radii[3] > 0.0);
    }

    #[test]
    fn active_nav_item_accent_is_inset_from_item_perimeter() {
        let mut nav = node("nav-overview", WidgetKind::NavItem);
        nav.style.parts.parts.insert(
            "item".to_string(),
            PartStyle {
                visual: VisualStyle {
                    border_radius: Some(8.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        nav.style.parts.parts.insert(
            "accent".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(5.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.11, 0.22, 0.33)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "nav-overview".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 36.0,
            },
        );
        let mut state = WidgetState::default();
        state
            .nav_targets
            .insert("nav-overview".to_string(), "overview".to_string());
        state
            .page_owner
            .insert("overview".to_string(), "pages".to_string());
        state
            .active_pages
            .insert("pages".to_string(), "overview".to_string());
        let mut out = Vec::new();

        emit_rects(
            &nav,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let accent = out
            .iter()
            .find(|inst| inst.color == [0.11, 0.22, 0.33, 1.0])
            .expect("active nav item accent should be emitted");
        assert_eq!(accent.rect, [2.0, 6.0, 5.0, 24.0]);
        assert_eq!(accent.radii, [2.5; 4]);

        let item_surface = out
            .iter()
            .find(|inst| inst.rect == [0.0, 0.0, 180.0, 36.0])
            .expect("active nav item surface should use the full item perimeter");
        assert_eq!(item_surface.radii, [8.0; 4]);
    }

    #[test]
    fn compact_nav_item_centers_icon_and_reduces_badge_to_indicator_dot() {
        let mut nav = node("nav-automation", WidgetKind::NavItem);
        nav.props
            .raw_props
            .insert("icon".to_string(), serde_json::json!("workflow"));
        nav.props.badge = Some("12".to_string());
        nav.style.parts.parts.insert(
            "badge".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.91, 0.12, 0.24)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "nav-automation".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 56.0,
                h: 36.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &nav,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        assert!(nav_item_uses_compact_icon(&nav, 56.0, 1.0));
        assert!(
            out.iter()
                .any(|instance| instance.rect == [43.0, 5.0, 8.0, 8.0]
                    && instance.color == [0.91, 0.12, 0.24, 1.0]
                    && instance.radii == [4.0; 4]),
            "compact badges should become a non-colliding indicator dot"
        );
        assert!(
            out.iter().any(|instance| {
                instance.rect[0] >= 19.0
                    && instance.rect[0] + instance.rect[2] <= 37.0
                    && instance.rect[1] >= 9.0
                    && instance.rect[1] + instance.rect[3] <= 27.0
            }),
            "workflow icon primitives should stay inside the centered 18px slot"
        );
    }

    #[test]
    fn focused_active_nav_item_uses_outline_focus_ring() {
        let mut nav = node("nav-overview", WidgetKind::NavItem);
        nav.style.parts.parts.insert(
            "item".to_string(),
            PartStyle {
                visual: VisualStyle {
                    border_radius: Some(8.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        nav.style.parts.parts.insert(
            "accent".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(5.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.11, 0.22, 0.33)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "nav-overview".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 36.0,
            },
        );
        let mut state = WidgetState::default();
        state.focused = Some("nav-overview".to_string());
        state
            .nav_targets
            .insert("nav-overview".to_string(), "overview".to_string());
        state
            .page_owner
            .insert("overview".to_string(), "pages".to_string());
        state
            .active_pages
            .insert("pages".to_string(), "overview".to_string());
        let theme = Theme::dark();
        let focus_color = with_alpha(theme.focus, 0.60);
        let mut out = Vec::new();

        emit_rects(
            &nav,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        let accent_index = out
            .iter()
            .position(|inst| inst.color == [0.11, 0.22, 0.33, 1.0])
            .expect("active nav item accent should be emitted");
        let focus_index = out
            .iter()
            .position(|inst| inst.color == focus_color)
            .expect("focused nav item should emit a focus ring");
        assert!(
            focus_index > accent_index,
            "focus ring should paint above the selected accent bar"
        );
        let focus = &out[focus_index];
        assert_eq!(focus.rect, [-2.0, -2.0, 184.0, 40.0]);
        assert_eq!(focus.radii, [10.0; 4]);
        assert_eq!(focus.params[2], 3.0);
        assert_eq!(focus.paint[3], 2.0);

        let accent = out
            .iter()
            .find(|inst| inst.color == [0.11, 0.22, 0.33, 1.0])
            .expect("active nav item accent should be emitted");
        assert_eq!(accent.rect, [2.0, 6.0, 5.0, 24.0]);

        assert!(!out.iter().any(|inst| {
            inst.color == focus_color
                && inst.params[2] == 0.0
                && inst.rect == [-2.0, -2.0, 184.0, 40.0]
        }));
    }

    #[test]
    fn top_level_menu_is_flat_until_interactive() {
        let menu = node("file-menu", WidgetKind::Menu);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "file-menu".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 64.0,
                h: 30.0,
            },
        );
        let theme = Theme::dark();
        let mut out = Vec::new();

        emit_rects(
            &menu,
            &layout,
            &theme,
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        assert!(
            out.is_empty(),
            "default closed menu should not render like a normal button"
        );

        let open_state = WidgetState {
            open_menu: Some("file-menu".to_string()),
            ..Default::default()
        };
        out.clear();
        emit_rects(
            &menu,
            &layout,
            &theme,
            1.0,
            &open_state,
            &HashMap::new(),
            &mut out,
        );
        assert!(
            out.is_empty(),
            "open top-level menu should leave the menu bar flat by default"
        );

        let hover_state = WidgetState {
            hovered: Some("file-menu".to_string()),
            ..Default::default()
        };
        out.clear();
        emit_rects(
            &menu,
            &layout,
            &theme,
            1.0,
            &hover_state,
            &HashMap::new(),
            &mut out,
        );
        assert!(
            out.is_empty(),
            "hovered top-level menu should rely on text color, not a button fill"
        );
    }

    #[test]
    fn menu_popup_border_paints_above_row_fills() {
        let mut root = node("root", WidgetKind::Window);
        let mut menu = node("file-menu", WidgetKind::Menu);
        menu.style.parts.parts.insert(
            "menu".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.05, 0.06, 0.07)),
                    border_color: Some(rgba(0.90, 0.10, 0.10)),
                    border_width: Some(2.0),
                    border_radius: Some(8.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        menu.style.parts.parts.insert(
            "item".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.10, 0.70, 0.20)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        root.children.push(menu);

        let items = vec![
            NavigationItem {
                id: "new".to_string(),
                value: "New".to_string(),
                disabled: false,
            },
            NavigationItem {
                id: "open".to_string(),
                value: "Open".to_string(),
                disabled: false,
            },
        ];
        let mut out = Vec::new();

        emit_menu_popup(
            &root,
            Rect {
                x: 10.0,
                y: 30.0,
                w: 140.0,
                h: 60.0,
            },
            &items,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            "file-menu",
            &mut out,
        );

        let last_row_fill = out
            .iter()
            .rposition(|inst| inst.color == [0.10, 0.70, 0.20, 1.0])
            .expect("menu row fill should be emitted");
        let border = out
            .iter()
            .enumerate()
            .rev()
            .find(|(_, inst)| inst.color == [0.90, 0.10, 0.10, 1.0] && inst.params[2] == 3.0)
            .expect("menu border ring should be emitted");
        assert!(
            border.0 > last_row_fill,
            "menu border ring should paint above row fills so rounded corners stay intact"
        );
        assert_eq!(border.1.rect, [10.0, 30.0, 140.0, 60.0]);
        assert_eq!(border.1.paint[3], 2.0);
    }

    #[test]
    fn overlay_renderers_consume_shared_scrim_tooltip_menu_dropdown_and_toast_colors() {
        let theme = Theme::dark();

        let mut modal = node("modal", WidgetKind::Modal);
        modal.props.open = Some(true);
        let mut modal_layout = LayoutResult::default();
        modal_layout.rects.insert(
            modal.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 140.0,
            },
        );
        let modal_state = WidgetState::from_tree(&modal);
        let mut modal_out = Vec::new();
        emit_rects(
            &modal,
            &modal_layout,
            &theme,
            1.0,
            &modal_state,
            &HashMap::new(),
            &mut modal_out,
        );
        let scrim = widget_part_paint_fallback(&modal, "scrim", &theme, &modal_state)
            .background
            .expect("modal scrim fallback");
        assert!(modal_out.iter().any(|instance| instance.color == scrim));

        let tooltip = node("tip", WidgetKind::Tooltip);
        let tooltip_state = WidgetState::default();
        let tooltip_fallback = widget_paint_fallback(&tooltip, &theme, &tooltip_state);
        let mut tooltip_out = Vec::new();
        emit_tooltip_surface(
            &tooltip,
            Rect {
                x: 10.0,
                y: 12.0,
                w: 120.0,
                h: 32.0,
            },
            &theme,
            1.0,
            &tooltip_state,
            &mut tooltip_out,
        );
        assert!(tooltip_out.iter().any(|instance| {
            instance.color
                == tooltip_fallback
                    .background
                    .expect("tooltip surface fallback")
        }));
        assert!(tooltip_out.iter().any(|instance| {
            instance.color
                == tooltip_fallback
                    .border_color
                    .expect("tooltip border fallback")
        }));

        let mut root = node("root", WidgetKind::Window);
        let menu = node("file-menu", WidgetKind::Menu);
        root.children.push(menu);
        let items = vec![
            NavigationItem {
                id: "base".to_string(),
                value: "Base".to_string(),
                disabled: false,
            },
            NavigationItem {
                id: "hover".to_string(),
                value: "Hover".to_string(),
                disabled: false,
            },
            NavigationItem {
                id: "disabled".to_string(),
                value: "Disabled".to_string(),
                disabled: true,
            },
        ];
        let menu_state = WidgetState {
            hovered: Some("hover".to_string()),
            ..Default::default()
        };
        let mut menu_out = Vec::new();
        emit_menu_popup(
            &root,
            Rect {
                x: 10.0,
                y: 30.0,
                w: 140.0,
                h: theme.control_height() * 3.0,
            },
            &items,
            &theme,
            1.0,
            &menu_state,
            "file-menu",
            &mut menu_out,
        );
        for part in ["menu", "item", "item-hover", "item-disabled"] {
            let color = native_widget_part_paint_fallback(
                WidgetKind::Menu,
                part,
                &theme,
                PaintInteraction::Resting,
                false,
            )
            .background
            .expect("menu part fallback");
            assert!(
                menu_out.iter().any(|instance| instance.color == color),
                "missing cataloged Menu::{part} color"
            );
        }

        let mut dropdown_root = node("dropdown-root", WidgetKind::Window);
        dropdown_root
            .children
            .push(node("mode", WidgetKind::Dropdown));
        let mut dropdown_layout = LayoutResult::default();
        dropdown_layout.rects.insert(
            "dropdown-root".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 360.0,
                h: 260.0,
            },
        );
        dropdown_layout.rects.insert(
            "mode".to_string(),
            Rect {
                x: 20.0,
                y: 20.0,
                w: 160.0,
                h: theme.control_height(),
            },
        );
        let mut dropdown_state = WidgetState {
            open_dropdown: Some("mode".to_string()),
            dropdown_hover: Some(("mode".to_string(), 1)),
            ..Default::default()
        };
        dropdown_state.dropdown_items.insert(
            "mode".to_string(),
            vec!["One".to_string(), "Two".to_string(), "Three".to_string()],
        );
        dropdown_state.dropdown_index.insert("mode".to_string(), 1);
        let mut dropdown_out = Vec::new();
        emit_dropdown_overlays(
            &dropdown_root,
            &dropdown_layout,
            &theme,
            1.0,
            &dropdown_state,
            &mut dropdown_out,
        );
        for fallback in [
            native_widget_part_paint_fallback(
                WidgetKind::Dropdown,
                "menu",
                &theme,
                PaintInteraction::Resting,
                false,
            ),
            native_widget_part_paint_fallback_with_selection(
                WidgetKind::Dropdown,
                "item",
                &theme,
                PaintInteraction::Resting,
                false,
                false,
            ),
            native_widget_part_paint_fallback_with_selection(
                WidgetKind::Dropdown,
                "item",
                &theme,
                PaintInteraction::Hovered,
                false,
                true,
            ),
        ] {
            let color = fallback.background.expect("dropdown overlay fallback");
            assert!(dropdown_out.iter().any(|instance| instance.color == color));
        }

        let toast = crate::toast::ToastOverlay {
            id: "failure".to_string(),
            message: "Failed".to_string(),
            level: crate::toast::ToastLevel::Error,
            opacity: 1.0,
            radius: None,
            padding: None,
            position: crate::toast::ToastPosition::TopRight,
        };
        let toast_fallback = native_widget_paint_fallback_with_level(
            WidgetKind::Toast,
            Some("error"),
            &theme,
            PaintInteraction::Resting,
        );
        let mut toast_out = Vec::new();
        emit_toast_overlays(
            &[toast],
            &theme,
            1.0,
            &StylesheetStore::default(),
            DgMediaEnvironment::new(400.0, 300.0),
            400.0,
            300.0,
            &mut toast_out,
        );
        for color in [
            toast_fallback.background.expect("toast fill fallback"),
            toast_fallback.border_color.expect("toast border fallback"),
        ] {
            assert!(toast_out.iter().any(|instance| instance.color == color));
        }
    }

    #[test]
    fn led_internal_parts_customize_dot_glow_and_highlight() {
        let mut led = node("status", WidgetKind::Led);
        led.props.led_state = Some("on".to_string());
        led.style.parts.parts.insert(
            "dot".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(12.0),
                    height: Some(10.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.10, 0.20, 0.30)),
                    border_width: Some(0.0),
                    border_radius: Some(2.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        led.style.parts.parts.insert(
            "glow".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(18.0),
                    height: Some(18.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.40, 0.50, 0.60)),
                    opacity: Some(0.25),
                    box_shadows: Some(Vec::new()),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        led.style.parts.parts.insert(
            "highlight".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(4.0),
                    height: Some(3.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.70, 0.80, 0.90)),
                    opacity: Some(0.5),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "status".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &led,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        assert!(has_rect(
            &out,
            [0.40, 0.50, 0.60, 0.25],
            [1.0, 1.0, 18.0, 18.0]
        ));
        assert!(has_rect(
            &out,
            [0.10, 0.20, 0.30, 1.0],
            [4.0, 5.0, 12.0, 10.0]
        ));
        assert!(out.iter().any(|inst| inst.color == [0.70, 0.80, 0.90, 0.5]));
    }

    #[test]
    fn drag_number_renderer_consumes_shared_grip_paint() {
        let drag_number = node("amount", WidgetKind::DragNumber);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            drag_number.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 36.0,
            },
        );
        let theme = Theme::dark();

        for interaction in [PaintInteraction::Resting, PaintInteraction::Disabled] {
            let mut state = WidgetState::from_tree(&drag_number);
            if interaction == PaintInteraction::Disabled {
                state.disabled.insert(drag_number.id.clone());
            }
            let mut out = Vec::new();
            emit_rects(
                &drag_number,
                &layout,
                &theme,
                1.0,
                &state,
                &HashMap::new(),
                &mut out,
            );

            let grip = native_widget_part_paint_fallback(
                WidgetKind::DragNumber,
                "grip",
                &theme,
                interaction,
                false,
            )
            .background
            .expect("drag number grip fallback");
            assert_eq!(
                out.iter().filter(|instance| instance.color == grip).count(),
                3,
                "DragNumber should draw all three grip marks with cataloged {interaction:?} paint"
            );
        }
    }

    #[test]
    fn number_input_renderer_consumes_shared_stepper_and_divider_paint() {
        let number = node("amount", WidgetKind::NumberInput);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            number.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 36.0,
            },
        );
        let theme = Theme::dark();

        for interaction in [PaintInteraction::Resting, PaintInteraction::Hovered] {
            let state = if interaction == PaintInteraction::Hovered {
                WidgetState {
                    hovered: Some(number.id.clone()),
                    ..Default::default()
                }
            } else {
                WidgetState::default()
            };
            let mut out = Vec::new();
            emit_rects(
                &number,
                &layout,
                &theme,
                1.0,
                &state,
                &HashMap::new(),
                &mut out,
            );

            let stepper = native_widget_part_paint_fallback(
                WidgetKind::NumberInput,
                "stepper",
                &theme,
                interaction,
                false,
            )
            .background
            .expect("number input stepper fallback");
            assert!(
                out.iter().any(|instance| instance.color == stepper),
                "missing cataloged {interaction:?} NumberInput stepper fill"
            );

            let divider = native_widget_part_paint_fallback(
                WidgetKind::NumberInput,
                "divider",
                &theme,
                PaintInteraction::Resting,
                false,
            )
            .background
            .expect("number input divider fallback");
            assert!(
                out.iter().any(|instance| {
                    instance.color == divider
                        && instance.params[2] == 0.0
                        && (instance.rect[2] <= 2.0 || instance.rect[3] <= 2.0)
                }),
                "missing cataloged NumberInput divider fill"
            );
        }
    }

    #[test]
    fn code_editor_and_dropdown_renderers_consume_field_and_caret_parts() {
        let theme = Theme::dark();

        let mut editor = node("editor", WidgetKind::CodeEditor);
        editor.style.parts.parts.insert(
            "field".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.12, 0.23, 0.34)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        editor.style.parts.parts.insert(
            "caret".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(3.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.91, 0.72, 0.13)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut editor_layout = LayoutResult::default();
        editor_layout.rects.insert(
            editor.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 90.0,
            },
        );
        let editor_state = WidgetState {
            focused: Some(editor.id.clone()),
            ..WidgetState::from_tree(&editor)
        };
        let mut editor_out = Vec::new();
        emit_rects(
            &editor,
            &editor_layout,
            &theme,
            1.0,
            &editor_state,
            &HashMap::new(),
            &mut editor_out,
        );
        assert!(editor_out
            .iter()
            .any(|instance| instance.color == [0.12, 0.23, 0.34, 1.0]));
        assert!(editor_out.iter().any(|instance| {
            instance.color == [0.91, 0.72, 0.13, 1.0] && instance.rect[2] == 3.0
        }));

        let mut dropdown = node("region", WidgetKind::Dropdown);
        dropdown.style.parts.parts.insert(
            "field".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.18, 0.42, 0.31)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut dropdown_layout = LayoutResult::default();
        dropdown_layout.rects.insert(
            dropdown.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 34.0,
            },
        );
        let dropdown_state = WidgetState::from_tree(&dropdown);
        let mut dropdown_out = Vec::new();
        emit_rects(
            &dropdown,
            &dropdown_layout,
            &theme,
            1.0,
            &dropdown_state,
            &HashMap::new(),
            &mut dropdown_out,
        );
        assert!(dropdown_out
            .iter()
            .any(|instance| instance.color == [0.18, 0.42, 0.31, 1.0]));
    }

    #[test]
    fn number_input_internal_parts_emit_distinct_primitives() {
        let mut number = node("amount", WidgetKind::NumberInput);
        number.style.visual.border_width = Some(1.0);
        number.style.parts.parts.insert(
            "field".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.10, 0.20, 0.30)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        number.style.parts.parts.insert(
            "divider".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(3.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.40, 0.50, 0.60)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        number.style.parts.parts.insert(
            "stepper-divider".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    height: Some(2.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.70, 0.80, 0.90)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        number.style.parts.parts.insert(
            "stepper-up".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(rgba(0.11, 0.22, 0.33)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        number.style.parts.parts.insert(
            "stepper-down".to_string(),
            PartStyle {
                text: TextStyle {
                    color: Some(rgba(0.44, 0.55, 0.66)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        number.style.parts.parts.insert(
            "caret".to_string(),
            PartStyle {
                layout: PartLayoutStyle {
                    width: Some(4.0),
                    height: Some(20.0),
                    ..Default::default()
                },
                visual: VisualStyle {
                    background: Some(rgba(0.90, 0.10, 0.20)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "amount".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 40.0,
            },
        );
        let mut state = WidgetState::default();
        state.focused = Some("amount".to_string());
        let mut caret_positions = HashMap::new();
        caret_positions.insert("amount".to_string(), [12.0, 0.0]);
        let mut out = Vec::new();

        emit_rects(
            &number,
            &layout,
            &Theme::dark(),
            1.0,
            &state,
            &caret_positions,
            &mut out,
        );

        assert!(
            has_rect(&out, [0.10, 0.20, 0.30, 1.0], [23.0, 1.0, 74.0, 38.0]),
            "number input primitives={:?}",
            out.iter()
                .map(|inst| (inst.color, inst.rect))
                .collect::<Vec<_>>()
        );
        assert!(has_rect(
            &out,
            [0.40, 0.50, 0.60, 1.0],
            [20.5, 1.0, 3.0, 38.0]
        ));
        assert!(has_rect(
            &out,
            [0.70, 0.80, 0.90, 1.0],
            [96.5, 1.0, 3.0, 38.0]
        ));
        assert!(has_rect(
            &out,
            [0.90, 0.10, 0.20, 1.0],
            [39.0, 10.0, 4.0, 20.0]
        ));
        assert_eq!(
            out.iter()
                .filter(|inst| inst.color == [0.11, 0.22, 0.33, 1.0])
                .count(),
            2
        );
        assert_eq!(
            out.iter()
                .filter(|inst| inst.color == [0.44, 0.55, 0.66, 1.0])
                .count(),
            1
        );
    }

    #[test]
    fn number_input_border_ring_paints_above_stepper_fills() {
        let mut number = node("amount", WidgetKind::NumberInput);
        number.style.visual.border_width = Some(2.0);
        number.style.visual.border_radius = Some(999.0);
        number.style.visual.border_color = Some(rgba(0.90, 0.10, 0.10));
        number.style.parts.parts.insert(
            "stepper".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(rgba(0.20, 0.30, 0.40)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "amount".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 36.0,
            },
        );
        let mut out = Vec::new();

        emit_rects(
            &number,
            &layout,
            &Theme::dark(),
            1.0,
            &WidgetState::default(),
            &HashMap::new(),
            &mut out,
        );

        let last_stepper_fill = out
            .iter()
            .rposition(|inst| inst.color == [0.20, 0.30, 0.40, 1.0] && inst.params[2] == 0.0)
            .expect("number input stepper fill should be emitted");
        let border = out
            .iter()
            .enumerate()
            .rev()
            .find(|(_, inst)| inst.color == [0.90, 0.10, 0.10, 1.0] && inst.params[2] == 3.0)
            .expect("number input border ring should be emitted");
        assert!(
            border.0 > last_stepper_fill,
            "number input border ring should paint above stepper fills so rounded caps stay intact"
        );
        assert_eq!(border.1.rect, [0.0, 0.0, 120.0, 36.0]);
        assert_eq!(border.1.paint[3], 2.0);
    }

    #[test]
    fn static_tooltip_surface_uses_tooltip_theme_not_target_widget_style() {
        let mut root = node("root", WidgetKind::Window);
        let mut button = node("upload", WidgetKind::Button);
        button.props.text = Some("Upload Buffer".to_string());
        button.props.tooltip = Some("Create a named native buffer resource.".to_string());
        button.style.visual.background = Some(rgba(1.0, 1.0, 1.0));
        button.style.visual.border_color = Some(rgba(1.0, 1.0, 1.0));
        root.children = vec![button];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "root".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 500.0,
                h: 300.0,
            },
        );
        layout.rects.insert(
            "upload".to_string(),
            Rect {
                x: 12.0,
                y: 20.0,
                w: 130.0,
                h: 36.0,
            },
        );
        let mut theme = Theme::dark();
        theme.surface_alt[3] = 0.24;
        theme.border[3] = 0.36;
        theme.accent[3] = 0.48;
        let mut state = WidgetState::default();
        state.hovered = Some("upload".to_string());
        let mut out = Vec::new();

        emit_tooltip_overlay(
            &root,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &StylesheetStore::default(),
            DgMediaEnvironment::new(500.0, 300.0),
            &mut out,
        );

        assert!(out
            .iter()
            .any(|inst| inst.color == with_alpha(theme.surface_alt, 1.0)));
        assert!(!out.iter().any(|inst| inst.color == [1.0, 1.0, 1.0, 1.0]));
        assert!(!out.iter().any(|inst| inst.color == theme.surface_alt));
    }

    #[test]
    fn standalone_badge_and_tag_emit_semantic_pills() {
        let mut root = node("row", WidgetKind::HLayout);
        let mut badge = node("badge", WidgetKind::Badge);
        badge.props.text = Some("live".to_string());
        badge.props.level = Some("success".to_string());
        let mut tag = node("tag", WidgetKind::Tag);
        tag.props.text = Some("queued".to_string());
        tag.props.level = Some("warning".to_string());
        root.children = vec![badge, tag];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "row".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 60.0,
            },
        );
        layout.rects.insert(
            "badge".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 48.0,
                h: 22.0,
            },
        );
        layout.rects.insert(
            "tag".to_string(),
            Rect {
                x: 56.0,
                y: 0.0,
                w: 72.0,
                h: 22.0,
            },
        );
        let theme = Theme::dark();
        let state = WidgetState::default();
        let badge_fallback = widget_paint_fallback(&root.children[0], &theme, &state);
        let tag_fallback = widget_paint_fallback(&root.children[1], &theme, &state);
        let mut out = Vec::new();

        emit_rects(
            &root,
            &layout,
            &theme,
            1.0,
            &state,
            &HashMap::new(),
            &mut out,
        );

        assert!(has_rect(
            &out,
            badge_fallback.background.expect("semantic badge fill"),
            [0.0, 0.0, 48.0, 22.0]
        ));
        assert!(has_rect(
            &out,
            tag_fallback.border_color.expect("semantic tag border"),
            [56.0, 0.0, 72.0, 22.0]
        ));
        assert!(has_rect(
            &out,
            tag_fallback.background.expect("semantic tag fill"),
            [57.0, 1.0, 70.0, 20.0]
        ));
    }
}
