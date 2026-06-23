use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ab_glyph::{point, Font as AbFont, FontArc, ScaleFont};
use base64::Engine;
use flate2::read::ZlibDecoder;
use glyphon::cosmic_text::{FeatureTag, FontFeatures};
use glyphon::{
    Attrs, Buffer, Cache, Color, ContentType, CustomGlyph, CustomGlyphId, Family, FontSystem,
    Metrics, RasterizeCustomGlyphRequest, RasterizedCustomGlyph, Resolution, Shaping,
    Style as GlyphStyle, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
    Weight, Wrap,
};
use serde_json::Value;

use crate::css_style::{
    computed_style_for_virtual_element_with_media, DgFontFaceSourceKind, DgMediaEnvironment,
    StylesheetStore,
};
use crate::document::{WidgetKind, WidgetNode};
use crate::events::{TableSortColumn, WidgetState};
use crate::layout::{tree_node_row_height_for_style, LayoutResult, Rect};
use crate::overlays::{
    active_menu_overlay_rects, active_tooltip_overlay_rect, dropdown_overlay_rect, find_node,
    menu_popup_rect, rich_tooltip_target, tooltip_target,
};
use crate::resources::ResourceRegistry;
use crate::style::{
    badge_font_size_lp, badge_height_for_style, badge_width_for_text, base_part_style,
    checked_part_style_for_state, code_editor_gutter_width_for_style,
    collapsed_part_style_for_state, collapsible_header_height_for_style,
    expanded_part_style_for_state, number_stepper_width_for_style, open_part_style_for_state,
    selected_part_style_for_state, standalone_badge_horizontal_padding_lp,
    state_part_style_for_state, uniform_layout_padding, ColorRef, FontFamily, FontStyle,
    FontVariantNumeric, GeneratedContent, LineHeight, NodeStyle, PartLayoutStyle, PartStyle,
    PositionStyle, TextAlign, TextOverflow, TextSpacing, TextStyle, TextTransform, TransformStyle,
    VisualStyle, BADGE_GAP_LP, BORDER_WIDTH_LP, CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP,
    DROPDOWN_CHEVRON_WIDTH_LP, TAB_GAP_LP, TOGGLE_SWITCH_TRACK_WIDTH_LP,
};
use crate::table;
use crate::theme::{parse_hex_color, parse_web_color, Theme};
use crate::toast::{toast_colors, toast_padding, toast_rect, toast_stack_index, ToastOverlay};

const LOADING_SPINNER_DEFAULT_SIZE_LP: f32 = 18.0;
const LOADING_SPINNER_GAP_LP: f32 = 8.0;

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

fn raw_prop_str<'a>(node: &'a WidgetNode, name: &str) -> Option<&'a str> {
    node.props
        .raw_props
        .get(name)
        .and_then(|value| value.as_str())
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

fn display_list_color(value: Option<&Value>, theme: &Theme, fallback: [f32; 4]) -> [f32; 4] {
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

fn display_list_scale(node: &WidgetNode, rect: Rect) -> (f32, f32) {
    let paint_w = node
        .props
        .raw_props
        .get("paint_width")
        .and_then(value_f32)
        .filter(|value| *value > 0.0)
        .or(node.props.intrinsic_width)
        .unwrap_or(rect.w.max(1.0));
    let paint_h = node
        .props
        .raw_props
        .get("paint_height")
        .and_then(value_f32)
        .filter(|value| *value > 0.0)
        .or(node.props.intrinsic_height)
        .unwrap_or(rect.h.max(1.0));
    (rect.w / paint_w.max(1.0), rect.h / paint_h.max(1.0))
}

fn display_list_text_align(value: Option<&Value>) -> TextAlign {
    match value.and_then(Value::as_str).unwrap_or("left") {
        "center" => TextAlign::Center,
        "right" => TextAlign::Right,
        _ => TextAlign::Left,
    }
}

fn icon_button_symbol_text(node: &WidgetNode) -> Option<&'static str> {
    match raw_prop_str(node, "icon")? {
        "help" | "question" => Some("?"),
        "warning" | "alert" => Some("!"),
        _ => None,
    }
}

fn loading_spinner_size_lp(node: &WidgetNode) -> f32 {
    raw_prop_f32(node, "size")
        .filter(|value| *value > 0.0)
        .unwrap_or(LOADING_SPINNER_DEFAULT_SIZE_LP)
}

// ---------------------------------------------------------------------------
// Layout constants (logical pixels × scale_factor = physical pixels)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Per-widget text entry (one Buffer per drawn text)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextKey {
    text: String,
    font_family: String,
    font_weight: u16,
    font_style: FontStyle,
    tabular_nums: bool,
    letter_spacing_milli: i32,
    font_size_milli: i32,
    line_height_milli: i32,
    width_milli: i32,
    wrap: bool,
}

struct TextEntry {
    key: TextKey,
    buffer: Buffer,
    left: f32,
    top: f32,
    scale: f32,
    clip: TextBounds,
    untransformed_clip: TextBounds,
    color: Color,
    custom_glyphs: Vec<CustomGlyph>,
}

type TextBufferCache = HashMap<TextKey, Vec<Buffer>>;
type FontFamilyAliases = HashMap<String, String>;

const OVERLAY_TEXT_BUFFER_CACHE_LIMIT: usize = 512;

fn text_buffer_cache_len(cache: &TextBufferCache) -> usize {
    cache.values().map(Vec::len).sum()
}

fn stash_text_buffer(cache: &mut TextBufferCache, key: TextKey, buffer: Buffer) {
    if text_buffer_cache_len(cache) >= OVERLAY_TEXT_BUFFER_CACHE_LIMIT {
        cache.clear();
    }
    cache.entry(key).or_default().push(buffer);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AxisLabelGlyphKey {
    text: String,
    font_size_milli: i32,
    rotate_ccw: bool,
}

#[derive(Clone)]
struct AxisLabelGlyphImage {
    width: u16,
    height: u16,
    data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
struct TextRenderOptions {
    font_style: Option<FontStyle>,
    font_variant_numeric: Option<FontVariantNumeric>,
    letter_spacing: Option<TextSpacing>,
    text_transform: Option<TextTransform>,
    text_overflow: Option<TextOverflow>,
}

fn text_options_from_style(style: &TextStyle) -> TextRenderOptions {
    TextRenderOptions {
        font_style: style.font_style,
        font_variant_numeric: style.font_variant_numeric,
        letter_spacing: style.letter_spacing,
        text_transform: style.text_transform,
        text_overflow: style.text_overflow,
    }
}

fn text_options_from_styles(
    primary: Option<&TextStyle>,
    fallback: &TextStyle,
) -> TextRenderOptions {
    let primary = primary.unwrap_or(fallback);
    TextRenderOptions {
        font_style: primary.font_style.or(fallback.font_style),
        font_variant_numeric: primary
            .font_variant_numeric
            .or(fallback.font_variant_numeric),
        letter_spacing: primary.letter_spacing.or(fallback.letter_spacing),
        text_transform: primary.text_transform.or(fallback.text_transform),
        text_overflow: primary.text_overflow.or(fallback.text_overflow),
    }
}

fn text_options_for_parts(node: &WidgetNode, parts: &[&str]) -> TextRenderOptions {
    let mut options = text_options_from_style(&node.style.text);
    if node.style.parts.is_empty() {
        return options;
    }
    for part in parts {
        if let Some(style) = base_part_style(&node.style, part) {
            options.font_style = style.text.font_style.or(options.font_style);
            options.font_variant_numeric = style
                .text
                .font_variant_numeric
                .or(options.font_variant_numeric);
            options.letter_spacing = style.text.letter_spacing.or(options.letter_spacing);
            options.text_transform = style.text.text_transform.or(options.text_transform);
            options.text_overflow = style.text.text_overflow.or(options.text_overflow);
        }
    }
    options
}

// ---------------------------------------------------------------------------
// TextRendererDg
// ---------------------------------------------------------------------------

pub struct TextRendererDg {
    font_system: FontSystem,
    attempted_font_sources: HashSet<String>,
    font_warnings: Vec<String>,
    font_aliases: FontFamilyAliases,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    renderer: TextRenderer,
    overlay_renderer: TextRenderer,
    viewport: Viewport,
    entries: Vec<TextEntry>,
    overlay_entry_start: usize,
    /// Ephemeral entries for scatter grid labels, cleared each frame.
    scatter_label_start: usize,
    scatter_label_cache: TextBufferCache,
    axis_label_glyph_ids: HashMap<AxisLabelGlyphKey, CustomGlyphId>,
    axis_label_glyph_images: HashMap<CustomGlyphId, AxisLabelGlyphImage>,
    next_axis_label_glyph_id: CustomGlyphId,
}

impl TextRendererDg {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        // Cache is cloned into TextAtlas and used for Viewport bind group;
        // it does not need to be retained after construction.
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, surface_format);
        let renderer = TextRenderer::new(
            &mut atlas,
            device,
            wgpu::MultisampleState::default(),
            // Text is a 2D overlay: no depth writes, always passes depth test.
            Some(wgpu::DepthStencilState {
                format: crate::DEPTH_STENCIL_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        );
        let overlay_renderer = TextRenderer::new(
            &mut atlas,
            device,
            wgpu::MultisampleState::default(),
            Some(wgpu::DepthStencilState {
                format: crate::DEPTH_STENCIL_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        );
        Self {
            font_system,
            attempted_font_sources: HashSet::new(),
            font_warnings: Vec::new(),
            font_aliases: HashMap::new(),
            swash_cache,
            atlas,
            renderer,
            overlay_renderer,
            viewport,
            entries: Vec::new(),
            overlay_entry_start: 0,
            scatter_label_start: 0,
            scatter_label_cache: HashMap::new(),
            axis_label_glyph_ids: HashMap::new(),
            axis_label_glyph_images: HashMap::new(),
            next_axis_label_glyph_id: 1,
        }
    }

    /// Update the viewport resolution (call on creation and every resize).
    pub fn update_screen(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.viewport.update(queue, Resolution { width, height });
    }

    /// Rebuild text buffers from the widget tree and layout.
    pub fn rebuild(
        &mut self,
        tree: &WidgetNode,
        layout: &LayoutResult,
        theme: &Theme,
        sf: f32,
        state: &WidgetState,
        resources: &ResourceRegistry,
        toasts: &[ToastOverlay],
        stylesheets: &StylesheetStore,
        media: DgMediaEnvironment,
    ) -> HashMap<String, [f32; 2]> {
        let pad = theme.spacing * sf;
        let open_dropdown = state.open_dropdown.as_deref();
        let dropdown_overlay = dropdown_overlay_rect(layout, state, theme, sf);
        let menu_overlays = active_menu_overlay_rects(tree, layout, state, theme, sf);
        let tooltip_overlay = active_tooltip_overlay_rect(tree, layout, theme, state, sf);
        let window_w = media.width * sf;
        let window_h = media.height * sf;
        self.sync_stylesheet_fonts(stylesheets);
        let active_modal = active_open_modal(tree);

        let mut entries = std::mem::take(&mut self.entries);
        let font_aliases = &self.font_aliases;
        let mut caret_positions = HashMap::new();
        let mut cache: TextBufferCache = HashMap::new();
        for entry in entries.drain(..) {
            cache.entry(entry.key).or_default().push(entry.buffer);
        }
        collect_text(
            tree,
            layout,
            state,
            theme,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            &[],
            true,
            &mut self.font_system,
            font_aliases,
            sf,
            pad,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );
        collect_table_text(
            tree,
            layout,
            state,
            resources,
            theme,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            &[],
            true,
            &mut self.font_system,
            font_aliases,
            sf,
            pad,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );
        self.overlay_entry_start = entries.len();
        if let Some(modal) = active_modal {
            collect_text(
                modal,
                layout,
                state,
                theme,
                open_dropdown,
                dropdown_overlay,
                menu_overlays,
                tooltip_overlay,
                &[],
                false,
                &mut self.font_system,
                font_aliases,
                sf,
                pad,
                &mut cache,
                &mut caret_positions,
                &mut entries,
            );
            collect_table_text(
                modal,
                layout,
                state,
                resources,
                theme,
                open_dropdown,
                dropdown_overlay,
                menu_overlays,
                tooltip_overlay,
                &[],
                false,
                &mut self.font_system,
                font_aliases,
                sf,
                pad,
                &mut cache,
                &mut caret_positions,
                &mut entries,
            );
        } else {
            collect_dropdown_overlay_text(
                tree,
                layout,
                state,
                theme,
                &mut self.font_system,
                font_aliases,
                sf,
                pad,
                &mut cache,
                &mut caret_positions,
                &mut entries,
            );
            collect_menu_overlay_text(
                tree,
                layout,
                state,
                theme,
                &mut self.font_system,
                font_aliases,
                sf,
                pad,
                &mut cache,
                &mut caret_positions,
                &mut entries,
            );
            collect_tooltip_text(
                tree,
                layout,
                state,
                theme,
                &mut self.font_system,
                font_aliases,
                sf,
                stylesheets,
                media,
                &mut cache,
                &mut caret_positions,
                &mut entries,
            );
            collect_rich_tooltip_text(
                tree,
                layout,
                state,
                theme,
                open_dropdown,
                dropdown_overlay,
                menu_overlays,
                &mut self.font_system,
                font_aliases,
                sf,
                pad,
                &mut cache,
                &mut caret_positions,
                &mut entries,
            );
        }
        collect_toast_text(
            toasts,
            theme,
            &mut self.font_system,
            font_aliases,
            sf,
            window_w,
            window_h,
            stylesheets,
            media,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );
        self.entries = entries;
        self.scatter_label_start = self.entries.len();
        caret_positions
    }

    /// Remove scatter grid labels added since the last `rebuild()` call.
    /// Call this each frame before `push_scatter_label`, just before `prepare()`.
    pub fn clear_scatter_labels(&mut self) {
        for entry in self.entries.drain(self.scatter_label_start..) {
            if entry.custom_glyphs.is_empty() {
                stash_text_buffer(&mut self.scatter_label_cache, entry.key, entry.buffer);
            }
        }
    }

    /// Append a scatter grid tick or axis-title label at a projected screen position.
    /// Labels are rendered in the overlay text layer (on top of scatter points).
    pub fn push_scatter_label(
        &mut self,
        text: &str,
        screen_x: f32,
        screen_y: f32,
        is_title: bool,
        clip: TextBounds,
        scale: f32,
        color_override: Option<[f32; 3]>,
        font_size_override: Option<f32>,
        anchor: &str,
    ) {
        if matches!(anchor, "plot-x-label" | "plot-y-label") {
            self.push_axis_label_glyph(
                text,
                screen_x,
                screen_y,
                clip,
                scale,
                color_override,
                font_size_override,
                anchor,
            );
            return;
        }

        let font_size = font_size_override.map(|s| s * scale).unwrap_or_else(|| {
            if is_title {
                13.0 * scale
            } else {
                11.0 * scale
            }
        });
        let line_height = font_size * 1.3;
        let avail_w = 160.0_f32 * scale;
        // Vertical anchors ("top"/"bottom") adjust the y offset; horizontal anchors
        // ("left"/"center"/"right") adjust the x offset and text alignment.
        // "top-left" is used by scatter overlays whose source renderer provides
        // final text-area origins instead of center anchors.
        let mut label_clip = clip;
        let (text_align, left, top) = match anchor {
            "top-left" => (TextAlign::Left, screen_x, screen_y),
            "plot-toolbar-button" => (
                TextAlign::Center,
                label_clip.left as f32,
                label_clip.top as f32
                    + ((label_clip.bottom - label_clip.top).max(1) as f32 - line_height).max(0.0)
                        * 0.5,
            ),
            "plot-readout" => (
                TextAlign::Center,
                label_clip.left as f32,
                label_clip.top as f32
                    + ((label_clip.bottom - label_clip.top).max(1) as f32 - line_height).max(0.0)
                        * 0.5,
            ),
            "box-center" => (
                TextAlign::Center,
                label_clip.left as f32,
                label_clip.top as f32
                    + ((label_clip.bottom - label_clip.top).max(1) as f32 - font_size).max(0.0)
                        * 0.5,
            ),
            "plot-x-tick" => {
                let width = 40.0 * scale;
                label_clip = intersect_text_bounds(
                    clip,
                    TextBounds {
                        left: (screen_x - width * 0.5).floor() as i32,
                        top: clip.top,
                        right: (screen_x + width * 0.5).ceil() as i32,
                        bottom: clip.bottom,
                    },
                );
                (TextAlign::Center, label_clip.left as f32, screen_y)
            }
            "plot-y-tick" => {
                let width = 30.0 * scale;
                label_clip = intersect_text_bounds(
                    clip,
                    TextBounds {
                        left: (screen_x - width).floor() as i32,
                        top: clip.top,
                        right: screen_x.ceil() as i32,
                        bottom: clip.bottom,
                    },
                );
                (
                    TextAlign::Right,
                    label_clip.left as f32,
                    screen_y - font_size * 0.5,
                )
            }
            "plot-y-category" => (
                TextAlign::Right,
                label_clip.left as f32,
                screen_y - font_size * 0.5,
            ),
            "top" => (TextAlign::Center, screen_x - avail_w * 0.5, screen_y),
            "bottom" => (
                TextAlign::Center,
                screen_x - avail_w * 0.5,
                screen_y - font_size,
            ),
            "left" => (TextAlign::Left, screen_x, screen_y - font_size * 0.5),
            "right" => (
                TextAlign::Right,
                screen_x - avail_w,
                screen_y - font_size * 0.5,
            ),
            _ => (
                TextAlign::Center,
                screen_x - avail_w * 0.5,
                screen_y - font_size * 0.5,
            ),
        };
        let color = text_color_from_rgb_override(color_override);
        let weight = if is_title || font_size_override.is_some() {
            600
        } else {
            400
        };
        let mut caret_positions = HashMap::new();
        push_text_entry(
            &mut self.font_system,
            &self.font_aliases,
            &mut self.entries,
            text,
            font_size,
            line_height,
            None,
            weight,
            left,
            top,
            label_clip,
            color,
            text_align,
            &mut self.scatter_label_cache,
            None,
            &mut caret_positions,
            TextRenderOptions::default(),
        );
    }

    pub fn push_overlay_panel_rect(
        &mut self,
        rect: [f32; 4],
        radius: f32,
        color: [f32; 4],
        clip: TextBounds,
    ) {
        if rect[2] <= 0.0 || rect[3] <= 0.0 {
            return;
        }
        let width = rect[2].ceil().clamp(1.0, u16::MAX as f32) as u16;
        let height = rect[3].ceil().clamp(1.0, u16::MAX as f32) as u16;
        let radius = radius.round().clamp(0.0, width.min(height) as f32 * 0.5) as u16;
        let panel_bounds = TextBounds {
            left: rect[0].floor() as i32,
            top: rect[1].floor() as i32,
            right: (rect[0] + width as f32).ceil() as i32,
            bottom: (rect[1] + height as f32).ceil() as i32,
        };
        let clip = intersect_text_bounds(clip, panel_bounds);
        if clip.right <= clip.left || clip.bottom <= clip.top {
            return;
        }
        let Some(glyph_id) = self.register_panel_rect_glyph(width, height, radius) else {
            return;
        };
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(1.0, 1.0));
        buffer.set_size(
            &mut self.font_system,
            Some(width as f32),
            Some(height as f32),
        );
        let key = TextKey {
            text: String::new(),
            font_family: String::new(),
            font_weight: Weight::NORMAL.0,
            font_style: FontStyle::Normal,
            tabular_nums: false,
            letter_spacing_milli: 0,
            font_size_milli: 1000,
            line_height_milli: 1000,
            width_milli: i32::from(width) * 1000,
            wrap: false,
        };
        self.entries.push(TextEntry {
            key,
            buffer,
            left: rect[0].floor(),
            top: rect[1].floor(),
            scale: 1.0,
            clip,
            untransformed_clip: clip,
            color: glyph_color(color),
            custom_glyphs: vec![CustomGlyph {
                id: glyph_id,
                left: 0.0,
                top: 0.0,
                width: width as f32,
                height: height as f32,
                color: None,
                snap_to_physical_pixel: true,
                metadata: 0,
            }],
        });
    }

    fn register_panel_rect_glyph(
        &mut self,
        width: u16,
        height: u16,
        radius: u16,
    ) -> Option<CustomGlyphId> {
        let key = AxisLabelGlyphKey {
            text: format!("__dg_panel_rect_{width}_{height}_{radius}"),
            font_size_milli: 0,
            rotate_ccw: false,
        };
        if let Some(id) = self.axis_label_glyph_ids.get(&key).copied() {
            return Some(id);
        }
        self.register_axis_label_glyph(key, panel_rect_glyph_image(width, height, radius))
    }

    #[allow(clippy::too_many_arguments)]
    fn push_axis_label_glyph(
        &mut self,
        text: &str,
        screen_x: f32,
        screen_y: f32,
        clip: TextBounds,
        scale: f32,
        color_override: Option<[f32; 3]>,
        font_size_override: Option<f32>,
        anchor: &str,
    ) {
        let text = text.trim();
        if text.is_empty() || clip.right <= clip.left || clip.bottom <= clip.top {
            return;
        }

        let font_size = font_size_override.unwrap_or(14.0) * scale;
        let rotate_ccw = anchor == "plot-y-label";
        let key = AxisLabelGlyphKey {
            text: text.to_string(),
            font_size_milli: (font_size * 1000.0).round() as i32,
            rotate_ccw,
        };
        let (glyph_id, image) = if let Some(id) = self.axis_label_glyph_ids.get(&key).copied() {
            let Some(image) = self.axis_label_glyph_images.get(&id).cloned() else {
                return;
            };
            (id, image)
        } else {
            let Some(image) = rasterize_axis_label_glyph(text, font_size, rotate_ccw) else {
                return;
            };
            let Some(id) = self.register_axis_label_glyph(key, image.clone()) else {
                return;
            };
            (id, image)
        };

        let width = image.width as f32;
        let height = image.height as f32;
        let (left, top) = match anchor {
            "plot-x-label" => (screen_x - width * 0.5, screen_y - height),
            "plot-y-label" => (screen_x - width * 0.5, screen_y - height * 0.5),
            _ => (screen_x, screen_y),
        };
        if left >= clip.right as f32
            || top >= clip.bottom as f32
            || left + width <= clip.left as f32
            || top + height <= clip.top as f32
        {
            return;
        }

        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(1.0, 1.0));
        buffer.set_size(
            &mut self.font_system,
            Some(width.max(1.0)),
            Some(height.max(1.0)),
        );
        let key = TextKey {
            text: String::new(),
            font_family: String::new(),
            font_weight: Weight::NORMAL.0,
            font_style: FontStyle::Normal,
            tabular_nums: false,
            letter_spacing_milli: 0,
            font_size_milli: 1000,
            line_height_milli: 1000,
            width_milli: (width * 1000.0).round() as i32,
            wrap: false,
        };
        self.entries.push(TextEntry {
            key,
            buffer,
            left,
            top,
            scale: 1.0,
            clip,
            untransformed_clip: clip,
            color: text_color_from_rgb_override(color_override),
            custom_glyphs: vec![CustomGlyph {
                id: glyph_id,
                left: 0.0,
                top: 0.0,
                width,
                height,
                color: None,
                snap_to_physical_pixel: true,
                metadata: 0,
            }],
        });
    }

    fn register_axis_label_glyph(
        &mut self,
        key: AxisLabelGlyphKey,
        image: AxisLabelGlyphImage,
    ) -> Option<CustomGlyphId> {
        if let Some(id) = self.axis_label_glyph_ids.get(&key) {
            return Some(*id);
        }
        if self.next_axis_label_glyph_id == CustomGlyphId::MAX {
            return None;
        }
        let id = self.next_axis_label_glyph_id;
        self.next_axis_label_glyph_id += 1;
        self.axis_label_glyph_ids.insert(key, id);
        self.axis_label_glyph_images.insert(id, image);
        Some(id)
    }

    fn sync_stylesheet_fonts(&mut self, stylesheets: &StylesheetStore) {
        for font_face in stylesheets.font_faces() {
            for source in &font_face.sources {
                let key = font_face_source_key(&font_face.family, source.kind, &source.url);
                if self.attempted_font_sources.contains(&key) {
                    continue;
                }
                self.attempted_font_sources.insert(key);
                match source.kind {
                    DgFontFaceSourceKind::Local => {
                        if let Some(actual_family) =
                            local_font_family_alias(&self.font_system, &source.url)
                        {
                            self.font_aliases
                                .insert(font_face.family.clone(), actual_family);
                            break;
                        }
                        self.record_font_warning(
                            &font_face.family,
                            &font_face_source_label(source.kind, &source.url),
                            "local font family was not found",
                        );
                    }
                    DgFontFaceSourceKind::Url => {
                        let source_label = font_face_source_label(source.kind, &source.url);
                        let resolved =
                            match resolve_font_source(&source.url, source.format.as_deref()) {
                                Ok(resolved) => resolved,
                                Err(message) => {
                                    self.record_font_warning(
                                        &font_face.family,
                                        &source_label,
                                        message,
                                    );
                                    continue;
                                }
                            };
                        let before = self.font_system.db().faces().count();
                        match resolved {
                            ResolvedFontSource::File(path) => {
                                if self.font_system.db_mut().load_font_file(&path).is_err() {
                                    self.record_font_warning(
                                        &font_face.family,
                                        &source_label,
                                        "failed to load font file",
                                    );
                                    continue;
                                }
                            }
                            ResolvedFontSource::Data(data) => {
                                self.font_system.db_mut().load_font_data(data);
                            }
                        }
                        let actual_family = {
                            self.font_system.db().faces().skip(before).find_map(|face| {
                                face.families.first().map(|(name, _)| name.clone())
                            })
                        };
                        if let Some(actual_family) = actual_family {
                            self.font_aliases
                                .insert(font_face.family.clone(), actual_family);
                        } else {
                            self.record_font_warning(
                                &font_face.family,
                                &source_label,
                                "font file loaded but exposed no usable font family",
                            );
                        }
                        break;
                    }
                }
            }
        }
    }

    fn record_font_warning(&mut self, family: &str, source: &str, message: &str) {
        let warning = format!("@font-face {family:?} source {source:?}: {message}");
        eprintln!("DragonGUI: {warning}");
        self.font_warnings.push(warning);
    }

    pub(crate) fn font_warnings(&self) -> &[String] {
        &self.font_warnings
    }

    /// Upload glyph data to the GPU.  Call this once per frame before `render`.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.entries.is_empty() {
            return;
        }

        // Destructure to obtain separate mutable/shared borrows of each field.
        let TextRendererDg {
            renderer,
            overlay_renderer,
            font_system,
            atlas,
            viewport,
            swash_cache,
            entries,
            overlay_entry_start,
            axis_label_glyph_images,
            ..
        } = self;

        let overlay_entry_start = (*overlay_entry_start).min(entries.len());
        let areas: Vec<TextArea<'_>> = entries[..overlay_entry_start]
            .iter()
            .map(|e| TextArea {
                buffer: &e.buffer,
                left: e.left,
                top: e.top,
                scale: e.scale,
                bounds: e.clip,
                default_color: e.color,
                custom_glyphs: &e.custom_glyphs,
            })
            .collect();

        if !areas.is_empty() {
            if let Err(e) = renderer.prepare_with_custom(
                device,
                queue,
                font_system,
                atlas,
                viewport,
                areas,
                swash_cache,
                |request| axis_label_custom_glyph_image(axis_label_glyph_images, request),
            ) {
                eprintln!("glyphon prepare error: {e}");
            }
        }

        let overlay_areas: Vec<TextArea<'_>> = entries[overlay_entry_start..]
            .iter()
            .map(|e| TextArea {
                buffer: &e.buffer,
                left: e.left,
                top: e.top,
                scale: e.scale,
                bounds: e.clip,
                default_color: e.color,
                custom_glyphs: &e.custom_glyphs,
            })
            .collect();

        if !overlay_areas.is_empty() {
            if let Err(e) = overlay_renderer.prepare_with_custom(
                device,
                queue,
                font_system,
                atlas,
                viewport,
                overlay_areas,
                swash_cache,
                |request| axis_label_custom_glyph_image(axis_label_glyph_images, request),
            ) {
                eprintln!("glyphon overlay prepare error: {e}");
            }
        }
    }

    pub fn render_base<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>) {
        if self.entries.is_empty() {
            return;
        }
        if self.overlay_entry_start.min(self.entries.len()) == 0 {
            return;
        }
        if let Err(e) = self.renderer.render(&self.atlas, &self.viewport, pass) {
            eprintln!("glyphon render error: {e}");
        }
    }

    pub fn render_overlays<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>) {
        if self.overlay_entry_start >= self.entries.len() {
            return;
        }
        if let Err(e) = self
            .overlay_renderer
            .render(&self.atlas, &self.viewport, pass)
        {
            eprintln!("glyphon overlay render error: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
fn text_color_from_rgb_override(color_override: Option<[f32; 3]>) -> Color {
    match color_override {
        Some([r, g, b]) => {
            let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
            Color::rgb(to_u8(r), to_u8(g), to_u8(b))
        }
        None => Color::rgb(0xbb, 0xbb, 0xbb),
    }
}

fn axis_label_custom_glyph_image(
    images: &HashMap<CustomGlyphId, AxisLabelGlyphImage>,
    request: RasterizeCustomGlyphRequest,
) -> Option<RasterizedCustomGlyph> {
    let image = images.get(&request.id)?;
    if image.width != request.width || image.height != request.height {
        return None;
    }
    Some(RasterizedCustomGlyph {
        data: image.data.clone(),
        content_type: ContentType::Mask,
    })
}

fn panel_rect_glyph_image(width: u16, height: u16, radius: u16) -> AxisLabelGlyphImage {
    let width_usize = width as usize;
    let height_usize = height as usize;
    let radius = (radius as f32)
        .min(width as f32 * 0.5)
        .min(height as f32 * 0.5);
    let mut data = vec![255_u8; width_usize.saturating_mul(height_usize)];
    if radius <= 0.0 || width == 0 || height == 0 {
        return AxisLabelGlyphImage {
            width,
            height,
            data,
        };
    }

    let samples = 4usize;
    let inv_samples = 1.0 / samples as f32;
    for y in 0..height_usize {
        for x in 0..width_usize {
            let mut covered = 0usize;
            for sy in 0..samples {
                for sx in 0..samples {
                    let px = x as f32 + (sx as f32 + 0.5) * inv_samples;
                    let py = y as f32 + (sy as f32 + 0.5) * inv_samples;
                    let cx = px.clamp(radius, width as f32 - radius);
                    let cy = py.clamp(radius, height as f32 - radius);
                    let dx = px - cx;
                    let dy = py - cy;
                    if dx * dx + dy * dy <= radius * radius {
                        covered += 1;
                    }
                }
            }
            data[y * width_usize + x] =
                ((covered as f32 / (samples * samples) as f32) * 255.0).round() as u8;
        }
    }

    AxisLabelGlyphImage {
        width,
        height,
        data,
    }
}

struct AxisLabelMask {
    width: usize,
    height: usize,
    alpha: Vec<u8>,
}

fn line_plot_axis_font() -> Option<&'static FontArc> {
    static FONT: OnceLock<Option<FontArc>> = OnceLock::new();
    FONT.get_or_init(|| {
        [
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ]
        .iter()
        .find_map(|path| {
            let bytes = std::fs::read(Path::new(path)).ok()?;
            FontArc::try_from_vec(bytes).ok()
        })
    })
    .as_ref()
}

fn rasterize_axis_label_glyph(
    label: &str,
    font_size_px: f32,
    rotate_ccw: bool,
) -> Option<AxisLabelGlyphImage> {
    let mask = rasterize_axis_label_mask(label, font_size_px)?;
    let (width, height, data) = if rotate_ccw {
        let width = mask.height;
        let height = mask.width;
        let mut data = vec![0_u8; width * height];
        for y in 0..height {
            for x in 0..width {
                let src_x = mask.width - 1 - y;
                let src_y = x;
                data[y * width + x] = mask.alpha[src_y * mask.width + src_x];
            }
        }
        (width, height, data)
    } else {
        (mask.width, mask.height, mask.alpha)
    };
    Some(AxisLabelGlyphImage {
        width: u16::try_from(width).ok()?,
        height: u16::try_from(height).ok()?,
        data,
    })
}

fn rasterize_axis_label_mask(label: &str, font_size_px: f32) -> Option<AxisLabelMask> {
    let text = label.trim();
    if text.is_empty() {
        return None;
    }

    let font = line_plot_axis_font()?;
    let font_size_px = font_size_px.max(6.0);
    let scaled = font.as_scaled(font_size_px);
    let baseline = scaled.ascent().ceil() + 1.0;
    let mut x = 0.0_f32;
    let mut previous = None;
    let mut glyphs = Vec::new();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for ch in text.chars().filter(|ch| !ch.is_control()) {
        let glyph_id = scaled.glyph_id(ch);
        if let Some(previous) = previous {
            x += scaled.kern(previous, glyph_id);
        }
        if ch.is_whitespace() {
            x += scaled.h_advance(glyph_id).max(font_size_px * 0.32);
            previous = Some(glyph_id);
            continue;
        }

        let glyph = glyph_id.with_scale_and_position(font_size_px, point(x, baseline));
        if let Some(outlined) = scaled.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            min_x = min_x.min(bounds.min.x);
            min_y = min_y.min(bounds.min.y);
            max_x = max_x.max(bounds.max.x);
            max_y = max_y.max(bounds.max.y);
            glyphs.push(outlined);
        }
        x += scaled.h_advance(glyph_id);
        previous = Some(glyph_id);
    }

    if glyphs.is_empty() || !min_x.is_finite() || !min_y.is_finite() {
        return None;
    }

    let origin_x = min_x.floor() as i32 - 1;
    let origin_y = min_y.floor() as i32 - 1;
    let width = (max_x.ceil() as i32 - origin_x + 1).max(1) as usize;
    let height = (max_y.ceil() as i32 - origin_y + 1).max(1) as usize;
    let mut alpha = vec![0_u8; width * height];

    for glyph in glyphs {
        let bounds = glyph.px_bounds();
        let base_x = bounds.min.x.floor() as i32 - origin_x;
        let base_y = bounds.min.y.floor() as i32 - origin_y;
        glyph.draw(|gx, gy, coverage| {
            let px = base_x + gx as i32;
            let py = base_y + gy as i32;
            if px < 0 || py < 0 {
                return;
            }
            let px = px as usize;
            let py = py as usize;
            if px >= width || py >= height {
                return;
            }
            let coverage = (coverage.clamp(0.0, 1.0) * 255.0).round() as u8;
            let target = &mut alpha[py * width + px];
            *target = (*target).max(coverage);
        });
    }

    Some(AxisLabelMask {
        width,
        height,
        alpha,
    })
}

// ---------------------------------------------------------------------------
fn font_face_source_key(family: &str, kind: DgFontFaceSourceKind, source: &str) -> String {
    format!("{family}|{kind:?}|{source}")
}

fn font_face_source_label(kind: DgFontFaceSourceKind, source: &str) -> String {
    match kind {
        DgFontFaceSourceKind::Local => format!("local({source})"),
        DgFontFaceSourceKind::Url => source.to_string(),
    }
}

fn local_font_family_alias(font_system: &FontSystem, requested: &str) -> Option<String> {
    font_system.db().faces().find_map(|face| {
        let first = face.families.first().map(|(name, _)| name.clone())?;
        face.families
            .iter()
            .any(|(name, _)| font_family_name_matches(name, requested))
            .then_some(first)
    })
}

fn font_family_name_matches(actual: &str, requested: &str) -> bool {
    actual == requested || actual.eq_ignore_ascii_case(requested)
}

enum ResolvedFontSource {
    File(PathBuf),
    Data(Vec<u8>),
}

fn resolve_font_source(
    url: &str,
    declared_format: Option<&str>,
) -> Result<ResolvedFontSource, &'static str> {
    if declared_format.is_some_and(|format| !is_supported_font_format(format)) {
        return Err(
            "unsupported font format; only truetype, opentype, collection, and woff are supported",
        );
    }
    if url
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        return decode_font_data_url(url).map(ResolvedFontSource::Data);
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return Err("remote font URLs are not supported");
    }
    if !is_supported_font_file_source(url, declared_format) {
        return Err("unsupported font source; only local(...), local .ttf, .otf, .ttc, and .woff files, and data: font URLs with sfnt or WOFF1 data are supported");
    }
    let path = font_source_path(url);
    if font_source_format(url, declared_format).is_some_and(|format| format == "woff") {
        let data = std::fs::read(&path).map_err(|_| "failed to read WOFF font file")?;
        return decode_woff_font_data(&data).map(ResolvedFontSource::Data);
    }
    Ok(ResolvedFontSource::File(path))
}

fn is_supported_font_file_source(url: &str, declared_format: Option<&str>) -> bool {
    if declared_format.is_some_and(is_supported_font_format) {
        return true;
    }
    matches!(
        font_source_extension(url).as_deref(),
        Some("ttf" | "otf" | "ttc" | "woff")
    )
}

fn font_source_format(url: &str, declared_format: Option<&str>) -> Option<String> {
    declared_format
        .and_then(normalize_font_format)
        .or_else(|| font_source_extension(url))
}

fn is_supported_font_format(format: &str) -> bool {
    matches!(
        normalize_font_format(format).as_deref(),
        Some("ttf" | "ttc" | "otf" | "truetype" | "opentype" | "collection" | "woff")
    )
}

fn normalize_font_format(format: &str) -> Option<String> {
    let format = format.trim().trim_matches('"').trim_matches('\'');
    if format.is_empty() {
        return None;
    }
    Some(format.to_ascii_lowercase())
}

fn font_source_extension(url: &str) -> Option<String> {
    let lower = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    Path::new(&lower)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_string)
}

fn font_source_path(url: &str) -> PathBuf {
    let mut path = url.split('?').next().unwrap_or(url).replace('\\', "/");
    if let Some(stripped) = path.strip_prefix("file:///") {
        path = stripped.to_string();
    } else if let Some(stripped) = path.strip_prefix("file://") {
        path = stripped.to_string();
    }
    path = percent_decode_path(&path);
    if cfg!(windows) && path.starts_with('/') && path.get(2..3) == Some(":") {
        path.remove(0);
    }
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(path),
            Err(_) => path,
        }
    }
}

fn percent_decode_path(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = hex_value(bytes[index + 1]);
            let lo = hex_value(bytes[index + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi << 4) | lo);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| value.to_string())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_font_data_url(url: &str) -> Result<Vec<u8>, &'static str> {
    let Some((metadata, payload)) = url[5..].split_once(',') else {
        return Err("invalid data: font URL");
    };
    if !metadata
        .split(';')
        .any(|part| part.eq_ignore_ascii_case("base64"))
    {
        return Err("unsupported data: font URL; only base64-encoded font data is supported");
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|_| "invalid base64 data in font URL")?;
    if is_supported_sfnt_font_data(&decoded) {
        return Ok(decoded);
    }
    if is_woff_font_data(&decoded) {
        return decode_woff_font_data(&decoded);
    }
    Err("unsupported font data; only sfnt TrueType/OpenType/TTC or WOFF1 data is supported")
}

fn is_supported_sfnt_font_data(data: &[u8]) -> bool {
    let Some(signature) = data.get(..4) else {
        return false;
    };
    signature == b"\0\x01\0\0"
        || signature == b"OTTO"
        || signature == b"ttcf"
        || signature == b"true"
}

fn is_woff_font_data(data: &[u8]) -> bool {
    data.get(..4) == Some(&b"wOFF"[..])
}

// Widget-tree → TextEntry mapping
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct WoffTable {
    tag: [u8; 4],
    checksum: u32,
    offset: u32,
    orig_len: u32,
    data: Vec<u8>,
}

fn decode_woff_font_data(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    if data.len() < 44 || !is_woff_font_data(data) {
        return Err("invalid WOFF font data");
    }
    let flavor = read_be_u32(data, 4).ok_or("invalid WOFF font data")?;
    let length = read_be_u32(data, 8).ok_or("invalid WOFF font data")? as usize;
    let num_tables = read_be_u16(data, 12).ok_or("invalid WOFF font data")? as usize;
    let total_sfnt_size = read_be_u32(data, 16).ok_or("invalid WOFF font data")? as usize;
    if length > data.len() || num_tables == 0 {
        return Err("invalid WOFF font data");
    }
    let directory_len = num_tables
        .checked_mul(20)
        .and_then(|value| 44usize.checked_add(value))
        .ok_or("invalid WOFF font data")?;
    if directory_len > length {
        return Err("invalid WOFF font data");
    }

    let mut tables = Vec::with_capacity(num_tables);
    for index in 0..num_tables {
        let entry = 44 + index * 20;
        let tag = data
            .get(entry..entry + 4)
            .and_then(|value| value.try_into().ok())
            .ok_or("invalid WOFF font data")?;
        let offset = read_be_u32(data, entry + 4).ok_or("invalid WOFF font data")?;
        let comp_len = read_be_u32(data, entry + 8).ok_or("invalid WOFF font data")?;
        let orig_len = read_be_u32(data, entry + 12).ok_or("invalid WOFF font data")?;
        let checksum = read_be_u32(data, entry + 16).ok_or("invalid WOFF font data")?;
        if comp_len > orig_len {
            return Err("invalid WOFF font data");
        }
        let start = offset as usize;
        let end = start
            .checked_add(comp_len as usize)
            .ok_or("invalid WOFF font data")?;
        let compressed = data.get(start..end).ok_or("invalid WOFF font data")?;
        let table_data = if comp_len == orig_len {
            compressed.to_vec()
        } else {
            let mut decoder = ZlibDecoder::new(compressed);
            let mut out = Vec::with_capacity(orig_len as usize);
            decoder
                .read_to_end(&mut out)
                .map_err(|_| "invalid compressed WOFF table data")?;
            if out.len() != orig_len as usize {
                return Err("invalid compressed WOFF table data");
            }
            out
        };
        tables.push(WoffTable {
            tag,
            checksum,
            offset,
            orig_len,
            data: table_data,
        });
    }

    tables.sort_by_key(|table| table.tag);
    let header_len = 12usize
        .checked_add(num_tables.checked_mul(16).ok_or("invalid WOFF font data")?)
        .ok_or("invalid WOFF font data")?;
    let mut next_offset = header_len;
    for table in &mut tables {
        next_offset = align4(next_offset).ok_or("invalid WOFF font data")?;
        table.offset = next_offset as u32;
        next_offset = next_offset
            .checked_add(table.orig_len as usize)
            .ok_or("invalid WOFF font data")?;
        next_offset = align4(next_offset).ok_or("invalid WOFF font data")?;
    }
    if total_sfnt_size != 0 && total_sfnt_size != next_offset {
        return Err("invalid WOFF font data");
    }

    let mut sfnt = Vec::with_capacity(next_offset);
    write_be_u32(&mut sfnt, flavor);
    write_be_u16(&mut sfnt, num_tables as u16);
    let max_power = 1usize << (usize::BITS - 1 - num_tables.leading_zeros());
    let search_range = max_power * 16;
    let entry_selector = max_power.trailing_zeros() as u16;
    let range_shift = num_tables * 16 - search_range;
    write_be_u16(&mut sfnt, search_range as u16);
    write_be_u16(&mut sfnt, entry_selector);
    write_be_u16(&mut sfnt, range_shift as u16);
    for table in &tables {
        sfnt.extend_from_slice(&table.tag);
        write_be_u32(&mut sfnt, table.checksum);
        write_be_u32(&mut sfnt, table.offset);
        write_be_u32(&mut sfnt, table.orig_len);
    }
    for table in &tables {
        while sfnt.len() < table.offset as usize {
            sfnt.push(0);
        }
        sfnt.extend_from_slice(&table.data);
        while sfnt.len() % 4 != 0 {
            sfnt.push(0);
        }
    }
    if !is_supported_sfnt_font_data(&sfnt) {
        return Err("invalid WOFF font data");
    }
    Ok(sfnt)
}

fn read_be_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_be_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn write_be_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_be_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn align4(value: usize) -> Option<usize> {
    value.checked_add(3).map(|value| value & !3)
}

fn collect_text(
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
    extra_overlays: &[Rect],
    skip_open_modals: bool,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
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
    let subtree_text_start = out.len();
    let primary_part_text = match node.kind {
        WidgetKind::ProgressBar => base_part_style(&node.style, "label").map(|part| &part.text),
        WidgetKind::LoadingSpinner => base_part_style(&node.style, "label").map(|part| &part.text),
        WidgetKind::Tab => base_part_style(&node.style, "tab").map(|part| &part.text),
        WidgetKind::NavItem => base_part_style(&node.style, "item").map(|part| &part.text),
        WidgetKind::Selectable => base_part_style(&node.style, "label").map(|part| &part.text),
        WidgetKind::RadioButton => base_part_style(&node.style, "label").map(|part| &part.text),
        WidgetKind::TreeNode => base_part_style(&node.style, "label").map(|part| &part.text),
        WidgetKind::ToggleSwitch => base_part_style(&node.style, "label").map(|part| &part.text),
        WidgetKind::DragNumber => base_part_style(&node.style, "value").map(|part| &part.text),
        WidgetKind::Collapsible => base_part_style(&node.style, "header").map(|part| &part.text),
        WidgetKind::IconButton => base_part_style(&node.style, "icon").map(|part| &part.text),
        _ => None,
    };
    let font_size = primary_part_text
        .and_then(|text| text.font_size)
        .map(|font_size| font_size.max(8.0) * sf)
        .unwrap_or_else(|| {
            if matches!(node.kind, WidgetKind::Badge | WidgetKind::Tag) {
                badge_font_size_lp(&node.style, theme) * sf
            } else if node.kind == WidgetKind::IconButton {
                (theme.font_size + 2.0).max(14.0) * sf
            } else {
                text_font_size(node, theme, sf)
            }
        });
    let line_height = if matches!(node.kind, WidgetKind::Badge | WidgetKind::Tag) {
        text_line_height_from_styles(primary_part_text, &node.style.text, font_size, theme, sf)
            .max(standalone_badge_line_height(node, font_size, theme, sf))
    } else {
        text_line_height_from_styles(primary_part_text, &node.style.text, font_size, theme, sf)
    };
    let font_family = primary_part_text
        .and_then(|text| text.font_family.as_ref())
        .or(node.style.text.font_family.as_ref());
    let font_weight = primary_part_text
        .and_then(|text| text.font_weight)
        .or(node.style.text.font_weight)
        .unwrap_or_else(|| {
            if node.kind == WidgetKind::IconButton {
                Weight::SEMIBOLD.0
            } else {
                Weight::NORMAL.0
            }
        });
    let align = primary_part_text
        .and_then(|text| text.text_align)
        .or(node.style.text.text_align)
        .unwrap_or(TextAlign::Left);
    let has_explicit_text_align = primary_part_text
        .and_then(|text| text.text_align)
        .or(node.style.text.text_align)
        .is_some();
    let text_options = text_options_from_styles(primary_part_text, &node.style.text);
    let is_text_widget = matches!(
        node.kind,
        WidgetKind::Panel
            | WidgetKind::Modal
            | WidgetKind::Sidebar
            | WidgetKind::Badge
            | WidgetKind::Tag
            | WidgetKind::Label
            | WidgetKind::Button
            | WidgetKind::SmallButton
            | WidgetKind::Selectable
            | WidgetKind::RadioButton
            | WidgetKind::TreeNode
            | WidgetKind::Checkbox
            | WidgetKind::ToggleSwitch
            | WidgetKind::Collapsible
            | WidgetKind::Dropdown
            | WidgetKind::Menu
            | WidgetKind::TextInput
            | WidgetKind::TextArea
            | WidgetKind::CodeEditor
            | WidgetKind::LogView
            | WidgetKind::NumberInput
            | WidgetKind::DragNumber
            | WidgetKind::ProgressBar
            | WidgetKind::LoadingSpinner
            | WidgetKind::Tab
            | WidgetKind::NavItem
            | WidgetKind::IconButton
            | WidgetKind::HtmlReport
    );
    if is_text_widget {
        let mut caret = None;
        if matches!(
            node.kind,
            WidgetKind::TextInput
                | WidgetKind::TextArea
                | WidgetKind::CodeEditor
                | WidgetKind::NumberInput
        ) {
            let value = state.text_for(&node.id).unwrap_or("");
            if value.is_empty() {
                caret_positions.insert(node.id.clone(), [0.0, 0.0]);
            } else {
                let cursor = state
                    .text_cursor
                    .get(&node.id)
                    .copied()
                    .unwrap_or(value.len());
                caret = Some((node.id.as_str(), cursor));
            }
        }

        if let (Some((text, placeholder)), Some(r)) =
            (display_text(node, state), layout.rects.get(&node.id))
        {
            if r.w > 0.0 && r.h > 0.0 {
                let node_clip = layout.visible_rect(&node.id);
                let (left, top, clip_left, clip_top, clip_right, clip_bottom) = match node.kind {
                    WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal => {
                        let title_pad = panel_title_padding(node, theme, sf);
                        let mut text_top = r.y + title_pad.top;
                        if node.kind == WidgetKind::Modal {
                            let border_w = node
                                .style
                                .visual
                                .border_width
                                .unwrap_or(BORDER_WIDTH_LP)
                                .max(0.0)
                                * sf;
                            let title_band_h = (title_pad.top + line_height).min(r.h);
                            text_top =
                                r.y + border_w + ((title_band_h - line_height) * 0.5).max(0.0);
                        }
                        (
                            r.x + title_pad.left,
                            text_top,
                            r.x + title_pad.left,
                            r.y,
                            r.x + r.w
                                - title_pad.right
                                - if node.kind == WidgetKind::Modal
                                    && raw_prop_bool(node, "close_button").unwrap_or(false)
                                {
                                    30.0 * sf
                                } else {
                                    0.0
                                },
                            text_top + line_height,
                        )
                    }
                    WidgetKind::Checkbox => {
                        let box_w = node
                            .style
                            .parts
                            .parts
                            .get("box")
                            .and_then(|part| part.layout.width)
                            .map(|width| width.max(1.0) * sf)
                            .unwrap_or(CHECKBOX_BOX_LP * sf);
                        let left = r.x + CHECKBOX_LEFT_PAD_LP * sf + box_w + pad;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (left, top, left, r.y, r.x + r.w - pad, r.y + r.h)
                    }
                    WidgetKind::Collapsible => {
                        let header_h =
                            collapsible_header_height_for_style(&node.style, theme, sf).min(r.h);
                        let indicator_w = node
                            .style
                            .parts
                            .parts
                            .get("indicator")
                            .and_then(|part| part.layout.width)
                            .unwrap_or(16.0)
                            .max(1.0)
                            * sf;
                        let header_pad = part_padding(node, &["header"], pad, sf);
                        let left = r.x + header_pad + indicator_w + pad * 0.75;
                        let top = r.y + ((header_h - line_height) * 0.5).max(0.0);
                        (left, top, left, r.y, r.x + r.w - header_pad, r.y + header_h)
                    }
                    WidgetKind::Dropdown => {
                        let chevron_w = dropdown_chevron_width(node, font_size, theme, sf);
                        let chevron_left = r.x + r.w - pad - chevron_w;
                        let top = centered_control_text_top(*r, line_height);
                        (
                            r.x + pad,
                            top,
                            r.x + pad,
                            r.y,
                            chevron_left - pad * 0.5,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::Button | WidgetKind::SmallButton => {
                        let reserved = badge_reserved_width(node, theme, sf);
                        let top = centered_control_text_top(*r, line_height);
                        (
                            r.x + pad,
                            top,
                            r.x + pad,
                            r.y,
                            r.x + r.w - pad - reserved,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::Selectable => {
                        let row_pad = part_padding(node, &["row"], pad, sf);
                        let indicator_slot = 14.0 * sf;
                        let left = r.x + row_pad + indicator_slot;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (left, top, left, r.y, r.x + r.w - row_pad, r.y + r.h)
                    }
                    WidgetKind::RadioButton => {
                        let indicator_w = node
                            .style
                            .parts
                            .parts
                            .get("indicator")
                            .and_then(|part| part.layout.width)
                            .unwrap_or(14.0)
                            .max(1.0)
                            * sf;
                        let left = r.x + pad + indicator_w + pad * 0.9;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (left, top, left, r.y, r.x + r.w - pad, r.y + r.h)
                    }
                    WidgetKind::ToggleSwitch => {
                        let track_w = node
                            .style
                            .parts
                            .parts
                            .get("track")
                            .and_then(|part| part.layout.width)
                            .map(|width| width.max(1.0) * sf)
                            .unwrap_or(TOGGLE_SWITCH_TRACK_WIDTH_LP * sf)
                            .min(r.w.max(1.0));
                        let label_left = node
                            .props
                            .raw_props
                            .get("label_position")
                            .and_then(|value| value.as_str())
                            .is_some_and(|value| value.eq_ignore_ascii_case("left"));
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        if label_left {
                            let track_x = r.x + r.w - CHECKBOX_LEFT_PAD_LP * sf - track_w;
                            let left = r.x + pad;
                            (left, top, left, r.y, (track_x - pad).max(left), r.y + r.h)
                        } else {
                            let left = r.x + CHECKBOX_LEFT_PAD_LP * sf + track_w + pad;
                            (left, top, left, r.y, r.x + r.w - pad, r.y + r.h)
                        }
                    }
                    WidgetKind::TreeNode => {
                        let indicator_w = node
                            .style
                            .parts
                            .parts
                            .get("indicator")
                            .and_then(|part| part.layout.width)
                            .unwrap_or(14.0)
                            .max(1.0)
                            * sf;
                        let row_h = tree_node_row_height_for_style(node, theme, sf, Some(r.h))
                            .min(r.h)
                            .max(1.0);
                        let row_pad = part_padding(node, &["row"], pad, sf);
                        let left = r.x + row_pad + indicator_w + pad * 0.75;
                        let top = r.y + ((row_h - line_height) * 0.5).max(0.0);
                        (left, top, left, r.y, r.x + r.w - row_pad, r.y + row_h)
                    }
                    WidgetKind::Badge | WidgetKind::Tag => {
                        let (pad_left, pad_right) =
                            standalone_badge_horizontal_padding_lp(&node.style);
                        let pad_left = pad_left * sf;
                        let pad_right = pad_right * sf;
                        let top = standalone_badge_text_top(*r, line_height);
                        (
                            r.x + pad_left,
                            top,
                            r.x + pad_left,
                            r.y,
                            r.x + r.w - pad_right,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::NumberInput => {
                        let step_w = number_stepper_width_for_style(&node.style, r.w, sf);
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        let text_left = r.x + step_w + pad;
                        (
                            text_left,
                            top,
                            text_left,
                            r.y,
                            r.x + r.w - step_w - pad * 0.5,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::DragNumber => {
                        let grip_w = node
                            .style
                            .parts
                            .parts
                            .get("grip")
                            .and_then(|part| part.layout.width)
                            .unwrap_or(16.0)
                            .max(1.0)
                            * sf;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        let text_left = r.x + pad;
                        (
                            text_left,
                            top,
                            text_left,
                            r.y,
                            r.x + r.w - grip_w - pad * 0.5,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::TextArea | WidgetKind::CodeEditor => {
                        let visible_h = (r.h - pad * 2.0).max(1.0);
                        let scroll_y = state.text_area_scroll_y(&node.id, visible_h, line_height);
                        let scroll_x = if node.props.wrap.unwrap_or(true) {
                            0.0
                        } else {
                            state.text_area_scroll_x(&node.id)
                        };
                        let gutter_w = if node.kind == WidgetKind::CodeEditor {
                            code_editor_gutter_width_for_style(&node.style, sf)
                                .min((r.w - pad * 2.0).max(1.0) * 0.5)
                        } else {
                            0.0
                        };
                        let top = r.y + pad;
                        (
                            r.x + pad + gutter_w - scroll_x,
                            top - scroll_y,
                            r.x + pad + gutter_w,
                            r.y + pad,
                            r.x + r.w - pad,
                            r.y + r.h - pad,
                        )
                    }
                    WidgetKind::LogView => {
                        let visible_h = (r.h - pad * 2.0).max(1.0);
                        let scroll_y = state.text_area_scroll_y(&node.id, visible_h, line_height);
                        let top = r.y + pad;
                        (
                            r.x + pad,
                            top - scroll_y,
                            r.x + pad,
                            r.y + pad,
                            r.x + r.w - pad,
                            r.y + r.h - pad,
                        )
                    }
                    WidgetKind::Tab => {
                        let scale = text_scale(font_size, theme);
                        let tab_pad = part_padding(node, &["tab"], pad, sf);
                        let reserved = badge_reserved_width(node, theme, sf);
                        let left = r.x + tab_pad + TAB_GAP_LP * scale * 0.5;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            left,
                            top,
                            left,
                            r.y,
                            r.x + r.w - tab_pad - TAB_GAP_LP * scale * 0.5 - reserved,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::ProgressBar => {
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (r.x + pad, top, r.x + pad, r.y, r.x + r.w - pad, r.y + r.h)
                    }
                    WidgetKind::LoadingSpinner => {
                        let spinner_size = loading_spinner_size_lp(node) * sf;
                        let gap = LOADING_SPINNER_GAP_LP * sf;
                        let left = r.x + spinner_size + gap;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (left, top, left, r.y, r.x + r.w - pad, r.y + r.h)
                    }
                    WidgetKind::IconButton => {
                        let top = centered_control_text_top(*r, line_height);
                        (r.x, top, r.x, r.y, r.x + r.w, r.y + r.h)
                    }
                    WidgetKind::NavItem => {
                        let item_pad = part_padding(node, &["item"], pad, sf);
                        let reserved = badge_reserved_width(node, theme, sf);
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            r.x + item_pad,
                            top,
                            r.x + item_pad,
                            r.y,
                            r.x + r.w - item_pad - reserved,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::Menu => {
                        let menu_pad = pad * 0.5;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            r.x + menu_pad,
                            top,
                            r.x + menu_pad,
                            r.y,
                            r.x + r.w - menu_pad,
                            r.y + r.h,
                        )
                    }
                    _ => {
                        let top = if label_text_should_top_align(
                            node,
                            text,
                            *r,
                            font_size,
                            line_height,
                            pad,
                        ) {
                            r.y
                        } else {
                            r.y + ((r.h - line_height) * 0.5).max(0.0)
                        };
                        (r.x + pad, top, r.x + pad, r.y, r.x + r.w - pad, r.y + r.h)
                    }
                };
                let color = if node.kind == WidgetKind::Dropdown && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["field"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::NumberInput && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["field"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::DragNumber && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["value", "field"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::CodeEditor && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["field"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::LogView && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["line"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if matches!(node.kind, WidgetKind::Checkbox | WidgetKind::ToggleSwitch)
                    && !placeholder
                {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["label"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::Selectable && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["label"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::RadioButton && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["label"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::TreeNode && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["label"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::Collapsible && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["header"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::ProgressBar && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["label"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::LoadingSpinner && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["label"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::IconButton && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["icon"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::Tab && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["tab"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if node.kind == WidgetKind::NavItem && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["item"],
                        text_color(node, state, theme, placeholder),
                    )
                } else if matches!(node.kind, WidgetKind::Badge | WidgetKind::Tag) && !placeholder {
                    standalone_badge_text_color(node, state, theme)
                } else {
                    text_color(node, state, theme, placeholder)
                };
                let align = if matches!(
                    node.kind,
                    WidgetKind::Button
                        | WidgetKind::SmallButton
                        | WidgetKind::IconButton
                        | WidgetKind::ProgressBar
                        | WidgetKind::Badge
                        | WidgetKind::Tag
                ) && !has_explicit_text_align
                {
                    TextAlign::Center
                } else {
                    align
                };
                let before_reserve =
                    generated_content_reserved_width(node, state, "before", theme, sf);
                let after_reserve =
                    generated_content_reserved_width(node, state, "after", theme, sf);
                let (left, clip_left, clip_right) = if before_reserve > 0.0 || after_reserve > 0.0 {
                    let adjusted_left = left + before_reserve;
                    let adjusted_clip_left = clip_left + before_reserve;
                    let adjusted_clip_right = (clip_right - after_reserve).max(adjusted_clip_left);
                    (adjusted_left, adjusted_clip_left, adjusted_clip_right)
                } else {
                    (left, clip_left, clip_right)
                };
                let clip_rect = node_clip.and_then(|node_clip| {
                    (Rect {
                        x: clip_left,
                        y: clip_top,
                        w: (clip_right - clip_left).max(0.0),
                        h: (clip_bottom - clip_top).max(0.0),
                    })
                    .intersect(node_clip)
                });
                if let Some(clip_rect) = clip_rect {
                    if !is_obscured_by_overlay(
                        node,
                        &clip_rect,
                        open_dropdown,
                        dropdown_overlay,
                        menu_overlays,
                        tooltip_overlay,
                        extra_overlays,
                    ) {
                        let text_bounds = TextBounds {
                            left: clip_rect.x as i32,
                            top: clip_rect.y as i32,
                            right: (clip_rect.x + clip_rect.w) as i32,
                            bottom: (clip_rect.y + clip_rect.h) as i32,
                        };
                        if node.kind == WidgetKind::LogView {
                            let scroll_y = layout
                                .rects
                                .get(&node.id)
                                .map(|r| {
                                    state.text_area_scroll_y(
                                        &node.id,
                                        (r.h - pad * 2.0).max(1.0),
                                        line_height,
                                    )
                                })
                                .unwrap_or(0.0);
                            emit_log_view_lines(
                                node,
                                state,
                                theme,
                                *r,
                                text,
                                scroll_y,
                                sf,
                                font_size,
                                line_height,
                                font_family,
                                font_weight,
                                text_bounds,
                                font_system,
                                font_aliases,
                                cache,
                                caret_positions,
                                out,
                            );
                        } else if matches!(node.kind, WidgetKind::TextArea | WidgetKind::CodeEditor)
                        {
                            let scroll_y = layout
                                .rects
                                .get(&node.id)
                                .map(|r| {
                                    state.text_area_scroll_y(
                                        &node.id,
                                        (r.h - pad * 2.0).max(1.0),
                                        line_height,
                                    )
                                })
                                .unwrap_or(0.0);
                            let scroll_x = if node.props.wrap.unwrap_or(true) {
                                0.0
                            } else {
                                state.text_area_scroll_x(&node.id)
                            };
                            if node.kind == WidgetKind::CodeEditor {
                                let raw_text = state.text_for(&node.id).unwrap_or("");
                                emit_code_editor_line_numbers(
                                    node,
                                    state,
                                    theme,
                                    *r,
                                    pad,
                                    sf,
                                    raw_text,
                                    line_height,
                                    font_size,
                                    font_family,
                                    font_weight,
                                    scroll_y,
                                    font_system,
                                    font_aliases,
                                    cache,
                                    caret_positions,
                                    out,
                                );
                            }
                            push_text_entry_impl(
                                font_system,
                                font_aliases,
                                out,
                                text,
                                font_size,
                                line_height,
                                font_family,
                                font_weight,
                                left,
                                top,
                                text_bounds,
                                color,
                                align,
                                node.props.wrap.unwrap_or(true),
                                scroll_x,
                                scroll_y,
                                cache,
                                if placeholder { None } else { caret },
                                caret_positions,
                                text_options,
                            );
                        } else if matches!(node.kind, WidgetKind::Label | WidgetKind::HtmlReport)
                            && node.props.wrap.unwrap_or(true)
                        {
                            push_wrapped_text_entry(
                                font_system,
                                font_aliases,
                                out,
                                text,
                                font_size,
                                line_height,
                                font_family,
                                font_weight,
                                left,
                                top,
                                text_bounds,
                                color,
                                align,
                                cache,
                                caret_positions,
                                text_options,
                            );
                        } else {
                            push_text_entry(
                                font_system,
                                font_aliases,
                                out,
                                text,
                                font_size,
                                line_height,
                                font_family,
                                font_weight,
                                left,
                                top,
                                text_bounds,
                                color,
                                align,
                                cache,
                                if placeholder { None } else { caret },
                                caret_positions,
                                text_options,
                            );
                        }
                    }
                }
            }
        }

        if matches!(
            node.kind,
            WidgetKind::Button | WidgetKind::SmallButton | WidgetKind::Tab | WidgetKind::NavItem
        ) {
            emit_badge_text(
                node,
                layout,
                theme,
                sf,
                state,
                font_system,
                font_aliases,
                out,
                cache,
                caret_positions,
                open_dropdown,
                dropdown_overlay,
                menu_overlays,
                tooltip_overlay,
                extra_overlays,
            );
        }
        emit_generated_content_text(
            node,
            "before",
            layout,
            state,
            theme,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            extra_overlays,
        );
        emit_generated_content_text(
            node,
            "after",
            layout,
            state,
            theme,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            extra_overlays,
        );
    }
    emit_extension_display_list_text(
        node,
        layout,
        theme,
        font_system,
        font_aliases,
        sf,
        cache,
        caret_positions,
        out,
    );

    visit_stacking_children(node, |child| {
        collect_text(
            child,
            layout,
            state,
            theme,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            extra_overlays,
            skip_open_modals,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    });
    if let Some(r) = layout.rects.get(&node.id) {
        apply_transform_to_text_entries(
            &mut out[subtree_text_start..],
            visual_transform_for_text(node, state),
            sf,
            [r.x + r.w * 0.5, r.y + r.h * 0.5],
        );
    }
    apply_paint_clip_to_text_entries(
        &mut out[subtree_text_start..],
        layout.paint_clip_rect(&node.id),
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_extension_display_list_text(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    if node.kind != WidgetKind::Extension {
        return;
    }
    let Some(Value::Array(commands)) = node.props.raw_props.get("display_list") else {
        return;
    };
    let Some(rect) = layout.rects.get(&node.id).copied() else {
        return;
    };
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let Some(visible) = layout.visible_rect(&node.id) else {
        return;
    };
    let Some(clip_rect) = rect.intersect(visible) else {
        return;
    };
    let clip = TextBounds {
        left: clip_rect.x.floor() as i32,
        top: clip_rect.y.floor() as i32,
        right: (clip_rect.x + clip_rect.w).ceil() as i32,
        bottom: (clip_rect.y + clip_rect.h).ceil() as i32,
    };
    let (sx, sy) = display_list_scale(node, rect);
    let text_scale = ((sx.abs() + sy.abs()) * 0.5).max(0.001);
    let base_font_size = text_font_size(node, theme, sf);
    let font_family = node.style.text.font_family.as_ref();
    let default_weight = node.style.text.font_weight.unwrap_or(Weight::NORMAL.0);
    let options = text_options_from_style(&node.style.text);

    for command in commands {
        let Some(command) = command.as_object() else {
            continue;
        };
        let cmd = command
            .get("cmd")
            .or_else(|| command.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if cmd != "text" {
            continue;
        }
        let Some(text) = command.get("text").and_then(Value::as_str) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let Some(local_x) = object_f32(command, "x") else {
            continue;
        };
        let Some(local_y) = object_f32(command, "y") else {
            continue;
        };
        let font_size = object_f32(command, "font_size")
            .map(|size| (size.max(1.0) * text_scale).max(1.0))
            .unwrap_or(base_font_size);
        let line_height = object_f32(command, "line_height")
            .map(|height| (height.max(1.0) * text_scale).max(font_size))
            .unwrap_or_else(|| text_line_height(font_size, theme, sf));
        let font_weight = command
            .get("font_weight")
            .and_then(Value::as_u64)
            .map(|weight| weight.clamp(1, 1000) as u16)
            .unwrap_or(default_weight);
        let left = rect.x + local_x * sx;
        let top = rect.y + local_y * sy;
        let color = glyph_color(display_list_color(
            command.get("fill").or_else(|| command.get("color")),
            theme,
            theme.text,
        ));
        push_text_entry(
            font_system,
            font_aliases,
            out,
            text,
            font_size,
            line_height,
            font_family,
            font_weight,
            left,
            top,
            clip,
            color,
            display_list_text_align(command.get("align")),
            cache,
            None,
            caret_positions,
            options,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_generated_content_text(
    node: &WidgetNode,
    part: &str,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    _pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
    extra_overlays: &[Rect],
) {
    let Some(style) = generated_part_style_for_state(node, state, part) else {
        return;
    };
    let Some(content) = style
        .content
        .as_ref()
        .and_then(|content| generated_content_text(node, content))
        .filter(|content| !content.is_empty())
    else {
        return;
    };
    let Some(r) = layout.rects.get(&node.id).copied() else {
        return;
    };
    if r.w <= 0.0 || r.h <= 0.0 {
        return;
    }
    let font_size = style
        .text
        .font_size
        .or(node.style.text.font_size)
        .map(|size| size.max(8.0) * sf)
        .unwrap_or_else(|| text_font_size(node, theme, sf));
    let line_height =
        text_line_height_from_styles(Some(&style.text), &node.style.text, font_size, theme, sf);
    let font_family = style
        .text
        .font_family
        .as_ref()
        .or(node.style.text.font_family.as_ref());
    let font_weight = style
        .text
        .font_weight
        .or(node.style.text.font_weight)
        .unwrap_or(Weight::NORMAL.0);
    let generated_pad = style.layout.padding.unwrap_or(theme.spacing * 0.5).max(0.0) * sf;
    let width = style.layout.width.map(|width| width.max(1.0) * sf);
    let left = if part == "after" {
        width
            .map(|width| (r.x + r.w - generated_pad - width).max(r.x + generated_pad))
            .unwrap_or(r.x + generated_pad)
    } else {
        r.x + generated_pad
    };
    let right = if part == "after" {
        r.x + r.w - generated_pad
    } else {
        width
            .map(|width| (left + width).min(r.x + r.w - generated_pad))
            .unwrap_or(r.x + r.w - generated_pad)
    };
    if right <= left {
        return;
    }
    let (top, clip_y, clip_bottom) =
        generated_content_vertical_bounds(node, r, line_height, theme, sf);
    let Some(clip_rect) = (Rect {
        x: left,
        y: clip_y,
        w: right - left,
        h: (clip_bottom - clip_y).max(0.0),
    })
    .intersect(layout.visible_rect(&node.id).unwrap_or(r)) else {
        return;
    };
    if is_obscured_by_overlay(
        node,
        &clip_rect,
        open_dropdown,
        dropdown_overlay,
        menu_overlays,
        tooltip_overlay,
        extra_overlays,
    ) {
        return;
    }
    let color = part_style_text_color(&style, theme)
        .unwrap_or_else(|| text_color(node, state, theme, false));
    let align = style.text.text_align.unwrap_or(if part == "after" {
        TextAlign::Right
    } else {
        TextAlign::Left
    });
    let bounds = TextBounds {
        left: clip_rect.x as i32,
        top: clip_rect.y as i32,
        right: (clip_rect.x + clip_rect.w) as i32,
        bottom: (clip_rect.y + clip_rect.h) as i32,
    };
    push_text_entry(
        font_system,
        font_aliases,
        out,
        &content,
        font_size,
        line_height,
        font_family,
        font_weight,
        left,
        top,
        bounds,
        color,
        align,
        cache,
        None,
        caret_positions,
        text_options_from_styles(Some(&style.text), &node.style.text),
    );
}

fn generated_content_vertical_bounds(
    node: &WidgetNode,
    r: Rect,
    line_height: f32,
    theme: &Theme,
    sf: f32,
) -> (f32, f32, f32) {
    if matches!(
        node.kind,
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
    ) {
        let title_pad = panel_title_padding(node, theme, sf);
        let top = r.y + title_pad.top;
        return (top, r.y, (top + line_height).min(r.y + r.h));
    }
    (r.y + ((r.h - line_height) * 0.5).max(0.0), r.y, r.y + r.h)
}

fn generated_content_text(node: &WidgetNode, content: &GeneratedContent) -> Option<String> {
    match content {
        GeneratedContent::Text(value) => Some(value.clone()),
        GeneratedContent::Attr(name) => widget_attr_value(node, name),
    }
}

fn generated_content_reserved_width(
    node: &WidgetNode,
    state: &WidgetState,
    part: &str,
    theme: &Theme,
    sf: f32,
) -> f32 {
    let Some(style) = generated_part_style_for_state(node, state, part) else {
        return 0.0;
    };
    let Some(content) = style
        .content
        .as_ref()
        .and_then(|content| generated_content_text(node, content))
        .filter(|content| !content.is_empty())
    else {
        return 0.0;
    };
    if let Some(width) = style.layout.width {
        return width.max(1.0) * sf;
    }
    let font_size = style
        .text
        .font_size
        .or(node.style.text.font_size)
        .map(|size| size.max(8.0) * sf)
        .unwrap_or_else(|| text_font_size(node, theme, sf));
    let padding = style.layout.padding.unwrap_or(theme.spacing * 0.5).max(0.0) * sf;
    let gap = style.layout.gap.unwrap_or(theme.spacing * 0.35).max(0.0) * sf;
    estimate_generated_text_width(&content, font_size) + padding * 2.0 + gap
}

fn estimate_generated_text_width(text: &str, font_size: f32) -> f32 {
    text.lines()
        .map(|line| line.chars().count() as f32)
        .fold(0.0, f32::max)
        * font_size
        * 0.56
}

fn widget_attr_value(node: &WidgetNode, name: &str) -> Option<String> {
    match name {
        "id" => Some(node.id.clone()),
        "key" => node.key.clone(),
        "class" | "class_" => node.class_name.clone(),
        "type" => Some(widget_kind_name(node.kind).to_string()),
        _ => node.props.raw_props.get(name).and_then(json_attr_value),
    }
    .filter(|value| !value.is_empty())
}

fn json_attr_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => Some(value.to_string()),
    }
}

fn widget_kind_name(kind: WidgetKind) -> &'static str {
    match kind {
        WidgetKind::Window => "window",
        WidgetKind::HLayout => "h_layout",
        WidgetKind::VLayout => "v_layout",
        WidgetKind::ScrollArea => "scroll_area",
        WidgetKind::GridLayout => "grid_layout",
        WidgetKind::FlowLayout => "flow_layout",
        WidgetKind::Splitter => "splitter",
        WidgetKind::Pane => "pane",
        WidgetKind::Panel => "panel",
        WidgetKind::Collapsible => "collapsible",
        WidgetKind::Modal => "modal",
        WidgetKind::Badge => "badge",
        WidgetKind::Tag => "tag",
        WidgetKind::Led => "led",
        WidgetKind::Button => "button",
        WidgetKind::SmallButton => "small_button",
        WidgetKind::IconButton => "icon_button",
        WidgetKind::ImageButton => "image_button",
        WidgetKind::ArrowButton => "arrow_button",
        WidgetKind::Selectable => "selectable",
        WidgetKind::RadioButton => "radio_button",
        WidgetKind::TreeView => "tree_view",
        WidgetKind::TreeNode => "tree_node",
        WidgetKind::DragSource => "drag_source",
        WidgetKind::DropTarget => "drop_target",
        WidgetKind::Checkbox => "checkbox",
        WidgetKind::ToggleSwitch => "toggle_switch",
        WidgetKind::Dropdown => "dropdown",
        WidgetKind::Label => "label",
        WidgetKind::Slider => "slider",
        WidgetKind::RangeSlider => "range_slider",
        WidgetKind::NumberInput => "number_input",
        WidgetKind::DragNumber => "drag_number",
        WidgetKind::ProgressBar => "progress_bar",
        WidgetKind::LoadingSpinner => "loading_spinner",
        WidgetKind::TextInput => "text_input",
        WidgetKind::TextArea => "text_area",
        WidgetKind::CodeEditor => "code_editor",
        WidgetKind::LogView => "log_view",
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
        WidgetKind::PieChart => "pie_chart",
        WidgetKind::Histogram => "histogram",
        WidgetKind::BarChart => "bar_chart",
        WidgetKind::Heatmap => "heatmap",
        WidgetKind::LinePlot => "line_plot",
        WidgetKind::Scatter3D => "scatter_3d",
        WidgetKind::DataFrameTable => "dataframe_table",
        WidgetKind::HtmlReport => "html_report",
        WidgetKind::Image => "image",
        WidgetKind::Extension => "extension",
        WidgetKind::Unknown => "unknown",
    }
}

fn generated_part_style_for_state(
    node: &WidgetNode,
    state: &WidgetState,
    part: &str,
) -> Option<PartStyle> {
    let mut style = PartStyle::default();
    let mut found = false;
    let mut merge = |overlay: Option<&PartStyle>| {
        if let Some(overlay) = overlay {
            merge_generated_part_style(&mut style, overlay);
            found = true;
        }
    };
    merge(base_part_style(&node.style, part));
    merge(checked_part_style_for_state(
        &node.style,
        &node.id,
        state,
        part,
    ));
    merge(open_part_style_for_state(
        &node.style,
        &node.id,
        state,
        part,
    ));
    merge(expanded_part_style_for_state(
        &node.style,
        &node.id,
        state,
        part,
    ));
    merge(collapsed_part_style_for_state(
        &node.style,
        &node.id,
        state,
        part,
    ));
    merge(selected_part_style_for_state(
        &node.style,
        &node.id,
        state,
        part,
    ));
    merge(state_part_style_for_state(
        &node.style,
        &node.id,
        state,
        part,
    ));
    found.then_some(style)
}

fn merge_generated_part_style(base: &mut PartStyle, overlay: &PartStyle) {
    merge_generated_part_layout(&mut base.layout, &overlay.layout);
    base.visual = base.visual.merged(&overlay.visual);
    merge_generated_text_style(&mut base.text, &overlay.text);
    base.content = overlay.content.clone().or_else(|| base.content.clone());
}

fn merge_generated_part_layout(base: &mut PartLayoutStyle, overlay: &PartLayoutStyle) {
    base.width = overlay.width.or(base.width);
    base.height = overlay.height.or(base.height);
    base.padding = overlay.padding.or(base.padding);
    base.gap = overlay.gap.or(base.gap);
}

fn merge_generated_text_style(base: &mut TextStyle, overlay: &TextStyle) {
    base.font_size = overlay.font_size.or(base.font_size);
    base.font_family = overlay
        .font_family
        .clone()
        .or_else(|| base.font_family.clone());
    base.font_weight = overlay.font_weight.or(base.font_weight);
    base.color = overlay.color.clone().or_else(|| base.color.clone());
    base.text_align = overlay.text_align.or(base.text_align);
    base.text_transform = overlay.text_transform.or(base.text_transform);
    base.letter_spacing = overlay.letter_spacing.or(base.letter_spacing);
    base.line_height = overlay.line_height.or(base.line_height);
    base.font_style = overlay.font_style.or(base.font_style);
    base.font_variant_numeric = overlay.font_variant_numeric.or(base.font_variant_numeric);
    base.text_overflow = overlay.text_overflow.or(base.text_overflow);
}

#[derive(Clone, Copy)]
struct PanelTitlePadding {
    left: f32,
    right: f32,
    top: f32,
}

fn panel_title_padding(node: &WidgetNode, theme: &Theme, sf: f32) -> PanelTitlePadding {
    let default = theme.spacing + 2.0;
    let layout = &node.style.layout;
    let all = layout.padding;
    PanelTitlePadding {
        left: layout.padding_left.or(all).unwrap_or(default) * sf,
        right: layout.padding_right.or(all).unwrap_or(default) * sf,
        top: layout.padding_top.or(all).unwrap_or(default) * sf,
    }
}

fn label_text_should_top_align(
    node: &WidgetNode,
    text: &str,
    rect: Rect,
    font_size: f32,
    line_height: f32,
    pad: f32,
) -> bool {
    if node.kind != WidgetKind::Label
        || rect.h <= line_height * 1.4
        || !node.props.wrap.unwrap_or(true)
    {
        return false;
    }
    if text.lines().count() > 1 {
        return true;
    }
    let available_width = (rect.w - pad * 2.0).max(font_size);
    let approx_char_width = (font_size * 0.56).max(1.0);
    text.chars().count() as f32 * approx_char_width > available_width + 1.0
}

fn centered_control_text_top(rect: Rect, line_height: f32) -> f32 {
    (rect.y + ((rect.h - line_height) * 0.5).max(0.0)).round()
}

fn standalone_badge_text_top(rect: Rect, line_height: f32) -> f32 {
    centered_control_text_top(rect, line_height)
}

fn is_obscured_by_overlay(
    node: &WidgetNode,
    r: &Rect,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    _menu_overlays: [Option<Rect>; 2],
    _tooltip_overlay: Option<Rect>,
    extra_overlays: &[Rect],
) -> bool {
    if open_dropdown == Some(node.id.as_str()) {
        return false;
    }
    dropdown_overlay.is_some_and(|overlay| rects_intersect(*r, overlay))
        || extra_overlays
            .iter()
            .any(|overlay| rects_intersect(*r, *overlay))
}

fn text_bounds_obscured_by_overlay(
    node: &WidgetNode,
    bounds: TextBounds,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
    extra_overlays: &[Rect],
) -> bool {
    let rect = Rect {
        x: bounds.left as f32,
        y: bounds.top as f32,
        w: (bounds.right - bounds.left).max(0) as f32,
        h: (bounds.bottom - bounds.top).max(0) as f32,
    };
    rect.w <= 0.0
        || rect.h <= 0.0
        || is_obscured_by_overlay(
            node,
            &rect,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            extra_overlays,
        )
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
}

fn active_open_modal(node: &WidgetNode) -> Option<&WidgetNode> {
    for child in &node.children {
        if let Some(modal) = active_open_modal(child) {
            return Some(modal);
        }
    }
    (node.kind == WidgetKind::Modal && node.props.open.unwrap_or(false)).then_some(node)
}

pub(crate) fn text_font_size(node: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
    node.style
        .text
        .font_size
        .unwrap_or(theme.font_size)
        .max(8.0)
        * sf
}

fn text_scale(font_size: f32, theme: &Theme) -> f32 {
    if theme.font_size > 0.0 {
        font_size / theme.font_size
    } else {
        1.0
    }
}

pub(crate) fn text_line_height(font_size: f32, theme: &Theme, sf: f32) -> f32 {
    (font_size + 5.0 * sf).max((theme.font_size + 3.0) * sf)
}

fn standalone_badge_line_height(node: &WidgetNode, font_size: f32, theme: &Theme, sf: f32) -> f32 {
    let height_lp = node
        .style
        .layout
        .height
        .unwrap_or_else(|| (badge_font_size_lp(&node.style, theme) + 8.0).max(20.0));
    (height_lp * sf).max(font_size + 2.0 * sf).max(1.0)
}

fn text_line_height_for_style(style: &TextStyle, font_size: f32, theme: &Theme, sf: f32) -> f32 {
    match style.line_height {
        Some(LineHeight::Multiplier(value)) => (font_size * value.max(0.1)).max(1.0),
        Some(LineHeight::LogicalPx(value)) => (value.max(1.0) * sf).max(1.0),
        None => text_line_height(font_size, theme, sf),
    }
}

fn text_line_height_from_styles(
    primary: Option<&TextStyle>,
    fallback: &TextStyle,
    font_size: f32,
    theme: &Theme,
    sf: f32,
) -> f32 {
    if let Some(style) = primary.filter(|style| style.line_height.is_some()) {
        text_line_height_for_style(style, font_size, theme, sf)
    } else {
        text_line_height_for_style(fallback, font_size, theme, sf)
    }
}

fn text_line_height_for_parts(
    node: &WidgetNode,
    parts: &[&str],
    font_size: f32,
    theme: &Theme,
    sf: f32,
) -> f32 {
    for part in parts {
        if let Some(style) = base_part_style(&node.style, part) {
            if style.text.line_height.is_some() {
                return text_line_height_for_style(&style.text, font_size, theme, sf);
            }
        }
    }
    text_line_height_for_style(&node.style.text, font_size, theme, sf)
}

fn dropdown_chevron_width(node: &WidgetNode, font_size: f32, theme: &Theme, sf: f32) -> f32 {
    node.style
        .parts
        .parts
        .get("chevron")
        .and_then(|part| part.layout.width)
        .map(|width| width.max(1.0) * sf)
        .unwrap_or_else(|| DROPDOWN_CHEVRON_WIDTH_LP * text_scale(font_size, theme))
}

fn text_color(node: &WidgetNode, state: &WidgetState, theme: &Theme, placeholder: bool) -> Color {
    let state_visual = state_visual_for(node, state);
    let base = if let Some(color) = state_visual
        .as_ref()
        .and_then(|visual| visual.foreground.as_ref())
    {
        color.resolve(theme)
    } else if state.is_disabled(&node.id) || placeholder {
        theme.muted_text
    } else if let Some(color) = &node.style.text.color {
        color.resolve(theme)
    } else if let Some(color) = &node.style.visual.foreground {
        color.resolve(theme)
    } else if matches!(node.kind, WidgetKind::Panel | WidgetKind::Sidebar) {
        theme.muted_text
    } else if (node.kind == WidgetKind::Tab && state.is_active_tab(&node.id))
        || (node.kind == WidgetKind::NavItem && state.is_active_nav_item(&node.id))
    {
        theme.accent
    } else {
        theme.text
    };
    let opacity = state_visual
        .as_ref()
        .and_then(|visual| visual.opacity)
        .or(node.style.visual.opacity)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    glyph_color([base[0], base[1], base[2], base[3] * opacity])
}

fn overlay_opacity(style: &NodeStyle, base_opacity: f32) -> f32 {
    (base_opacity * style.visual.opacity.unwrap_or(1.0)).clamp(0.0, 1.0)
}

fn overlay_text_color(style: &NodeStyle, theme: &Theme, fallback: [f32; 4], opacity: f32) -> Color {
    let mut color = style
        .text
        .color
        .as_ref()
        .or(style.visual.foreground.as_ref())
        .map(|color| color.resolve(theme))
        .unwrap_or(fallback);
    color[3] *= opacity.clamp(0.0, 1.0);
    glyph_color(color)
}

fn standalone_badge_level_color(node: &WidgetNode, theme: &Theme) -> [f32; 4] {
    match node.props.level.as_deref().unwrap_or("info") {
        "success" => theme.success,
        "warning" => theme.warning,
        "danger" | "error" => theme.danger,
        "neutral" => theme.muted_text,
        _ => theme.accent,
    }
}

fn mix_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn standalone_badge_fill(node: &WidgetNode, theme: &Theme) -> [f32; 4] {
    let semantic = standalone_badge_level_color(node, theme);
    if node.kind == WidgetKind::Tag {
        mix_color(theme.surface_alt, semantic, 0.22)
    } else {
        semantic
    }
}

fn standalone_badge_text_color(node: &WidgetNode, state: &WidgetState, theme: &Theme) -> Color {
    if state.is_disabled(&node.id) {
        return glyph_color(theme.disabled);
    }
    if let Some(color) = node
        .style
        .text
        .color
        .as_ref()
        .or(node.style.visual.foreground.as_ref())
    {
        return glyph_color(color.resolve(theme));
    }
    let fill = node
        .style
        .visual
        .background
        .as_ref()
        .map(|color| color.resolve(theme))
        .unwrap_or_else(|| standalone_badge_fill(node, theme));
    contrast_glyph_color(fill)
}

fn state_visual_for(node: &WidgetNode, state: &WidgetState) -> Option<VisualStyle> {
    let mut visual = VisualStyle::default();
    let mut changed = false;
    if state.checked.get(&node.id).copied().unwrap_or(false) {
        visual = visual.merged(&node.style.checked);
        changed = true;
    }
    if node_is_open(node, state) {
        visual = visual.merged(&node.style.open);
        changed = true;
    }
    if state.is_expanded_widget(&node.id) {
        visual = visual.merged(&node.style.expanded);
        changed = true;
    }
    if state.is_collapsed_widget(&node.id) {
        visual = visual.merged(&node.style.collapsed);
        changed = true;
    }
    if state.is_selected_widget(&node.id) {
        visual = visual.merged(&node.style.selected);
        changed = true;
    }
    if state.pressed.as_deref() == Some(node.id.as_str()) {
        visual = visual.merged(&node.style.active);
        changed = true;
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        visual = visual.merged(&node.style.hover);
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
    changed.then_some(visual)
}

fn visual_transform_for_text(node: &WidgetNode, state: &WidgetState) -> Option<TransformStyle> {
    let mut visual = node.style.visual.clone();
    if let Some(state_visual) = state_visual_for(node, state) {
        visual = visual.merged(&state_visual);
    }
    paint_transform_for_text(node, visual.transform)
}

fn paint_transform_for_text(
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

fn apply_transform_to_text_entries(
    entries: &mut [TextEntry],
    transform: Option<TransformStyle>,
    sf: f32,
    origin: [f32; 2],
) {
    let Some(transform) = transform.filter(|transform| {
        transform.translate_x != 0.0
            || transform.translate_y != 0.0
            || transform.scale_x != 1.0
            || transform.scale_y != 1.0
    }) else {
        return;
    };
    let dx = transform.translate_x * sf;
    let dy = transform.translate_y * sf;
    let text_scale = ((transform.scale_x.abs() + transform.scale_y.abs()) * 0.5).max(0.01);
    for entry in entries {
        entry.left = origin[0] + (entry.left - origin[0]) * text_scale + dx;
        entry.top = origin[1] + (entry.top - origin[1]) * text_scale + dy;
        entry.scale *= text_scale;
        entry.clip = transform_text_clip_to_painted_position(
            entry.clip,
            entry.untransformed_clip,
            origin,
            text_scale,
            dx,
            dy,
        );
    }
}

fn apply_paint_clip_to_text_entries(entries: &mut [TextEntry], clip: Option<Rect>) {
    let Some(clip) = clip else {
        return;
    };
    let clip_bounds = TextBounds {
        left: clip.x.floor() as i32,
        top: clip.y.floor() as i32,
        right: (clip.x + clip.w).ceil() as i32,
        bottom: (clip.y + clip.h).ceil() as i32,
    };
    for entry in entries {
        entry.clip = intersect_text_bounds(entry.clip, clip_bounds);
    }
}

fn intersect_text_bounds(a: TextBounds, b: TextBounds) -> TextBounds {
    let left = a.left.max(b.left);
    let top = a.top.max(b.top);
    let right = a.right.min(b.right);
    let bottom = a.bottom.min(b.bottom);
    if right <= left || bottom <= top {
        return TextBounds {
            left,
            top,
            right: left,
            bottom: top,
        };
    }
    TextBounds {
        left,
        top,
        right,
        bottom,
    }
}

fn transform_text_clip_to_painted_position(
    current_clip: TextBounds,
    untransformed_clip: TextBounds,
    origin: [f32; 2],
    scale: f32,
    dx: f32,
    dy: f32,
) -> TextBounds {
    let mut transformed = transformed_text_bounds(untransformed_clip, origin, scale, dx, dy);
    if current_clip.left > untransformed_clip.left {
        transformed.left = transformed.left.max(current_clip.left);
    }
    if current_clip.top > untransformed_clip.top {
        transformed.top = transformed.top.max(current_clip.top);
    }
    if current_clip.right < untransformed_clip.right {
        transformed.right = transformed.right.min(current_clip.right);
    }
    if current_clip.bottom < untransformed_clip.bottom {
        transformed.bottom = transformed.bottom.min(current_clip.bottom);
    }
    transformed
}

fn transformed_text_bounds(
    bounds: TextBounds,
    origin: [f32; 2],
    scale: f32,
    dx: f32,
    dy: f32,
) -> TextBounds {
    let left = origin[0] + (bounds.left as f32 - origin[0]) * scale + dx;
    let right = origin[0] + (bounds.right as f32 - origin[0]) * scale + dx;
    let top = origin[1] + (bounds.top as f32 - origin[1]) * scale + dy;
    let bottom = origin[1] + (bounds.bottom as f32 - origin[1]) * scale + dy;
    TextBounds {
        left: left.min(right).floor() as i32,
        top: top.min(bottom).floor() as i32,
        right: left.max(right).ceil() as i32,
        bottom: top.max(bottom).ceil() as i32,
    }
}

fn node_is_open(node: &WidgetNode, state: &WidgetState) -> bool {
    state.is_open_widget(&node.id)
        || (node.kind == WidgetKind::Modal && node.props.open == Some(true))
}

fn part_style_text_color(style: &crate::style::PartStyle, theme: &Theme) -> Option<Color> {
    let color = style
        .text
        .color
        .as_ref()
        .or(style.visual.foreground.as_ref())?;
    let mut resolved = color.resolve(theme);
    if let Some(opacity) = style.visual.opacity {
        resolved[3] *= opacity.clamp(0.0, 1.0);
    }
    Some(glyph_color(resolved))
}

fn part_text_color(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    parts: &[&str],
    default: Color,
) -> Color {
    let fallback = if state.is_disabled(&node.id) {
        glyph_color(theme.disabled)
    } else {
        default
    };
    if node.style.parts.is_empty() {
        return fallback;
    }
    for part in parts {
        if let Some(color) = state_part_style_for_state(&node.style, &node.id, state, *part)
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = checked_part_style_for_state(&node.style, &node.id, state, *part)
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = open_part_style_for_state(&node.style, &node.id, state, *part)
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = expanded_part_style_for_state(&node.style, &node.id, state, *part)
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = collapsed_part_style_for_state(&node.style, &node.id, state, *part)
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = selected_part_style_for_state(&node.style, &node.id, state, *part)
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    for part in parts {
        if let Some(color) = base_part_style(&node.style, *part)
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    fallback
}

fn part_text_style<'a>(
    node: &'a WidgetNode,
    parts: &[&str],
) -> Option<&'a crate::style::TextStyle> {
    if node.style.parts.is_empty() {
        return None;
    }
    parts
        .iter()
        .find_map(|part| base_part_style(&node.style, *part).map(|style| &style.text))
}

fn part_font_size(node: &WidgetNode, parts: &[&str], fallback: f32, sf: f32) -> f32 {
    part_text_style(node, parts)
        .and_then(|style| style.font_size)
        .map(|font_size| font_size.max(8.0) * sf)
        .unwrap_or(fallback)
}

fn part_font_family<'a>(
    node: &'a WidgetNode,
    parts: &[&str],
    fallback: Option<&'a FontFamily>,
) -> Option<&'a FontFamily> {
    part_text_style(node, parts)
        .and_then(|style| style.font_family.as_ref())
        .or(fallback)
}

fn part_font_weight(node: &WidgetNode, parts: &[&str], fallback: u16) -> u16 {
    part_text_style(node, parts)
        .and_then(|style| style.font_weight)
        .unwrap_or(fallback)
}

fn part_padding(node: &WidgetNode, parts: &[&str], default: f32, sf: f32) -> f32 {
    for part in parts {
        if let Some(padding) =
            base_part_style(&node.style, *part).and_then(|style| style.layout.padding)
        {
            return (padding.max(0.0) * sf).max(0.0);
        }
    }
    default
}

#[allow(clippy::too_many_arguments)]
fn emit_badge_text(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    out: &mut Vec<TextEntry>,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
    extra_overlays: &[Rect],
) {
    let Some(badge) = node
        .props
        .badge
        .as_deref()
        .filter(|badge| !badge.is_empty())
    else {
        return;
    };
    let Some(r) = layout.rects.get(&node.id) else {
        return;
    };
    let Some(node_clip) = layout.visible_rect(&node.id) else {
        return;
    };
    let right_inset = if node.kind == WidgetKind::Tab {
        TAB_GAP_LP * sf
    } else {
        theme.spacing * sf
    };
    let Some(rect) = badge_rect(node, *r, theme, sf, right_inset) else {
        return;
    };
    let Some(clip_rect) = rect.intersect(node_clip) else {
        return;
    };
    if is_obscured_by_overlay(
        node,
        &clip_rect,
        open_dropdown,
        dropdown_overlay,
        menu_overlays,
        tooltip_overlay,
        extra_overlays,
    ) {
        return;
    }

    let badge_font_size = badge_font_size_lp(&node.style, theme) * sf;
    let badge_parts = &["badge"];
    let badge_line_height =
        text_line_height_for_parts(node, badge_parts, badge_font_size, theme, sf).max(rect.h);
    let visual = crate::style::part_visual_for_state(&node.style, &node.id, state, "badge");
    let bg = visual
        .background
        .as_ref()
        .or(visual.foreground.as_ref())
        .map(|color| color.resolve(theme))
        .unwrap_or(theme.accent);
    let color = part_text_color(node, state, theme, &["badge"], contrast_glyph_color(bg));
    push_text_entry(
        font_system,
        font_aliases,
        out,
        badge,
        badge_font_size,
        badge_line_height,
        part_font_family(node, badge_parts, node.style.text.font_family.as_ref()),
        part_font_weight(
            node,
            badge_parts,
            node.style.text.font_weight.unwrap_or(Weight::BOLD.0),
        ),
        rect.x,
        rect.y + ((rect.h - badge_line_height) * 0.5).max(0.0),
        TextBounds {
            left: clip_rect.x as i32,
            top: clip_rect.y as i32,
            right: (clip_rect.x + clip_rect.w) as i32,
            bottom: (clip_rect.y + clip_rect.h) as i32,
        },
        color,
        TextAlign::Center,
        cache,
        None,
        caret_positions,
        text_options_for_parts(node, badge_parts),
    );
}

fn badge_rect(
    node: &WidgetNode,
    rect: Rect,
    theme: &Theme,
    sf: f32,
    right_inset: f32,
) -> Option<Rect> {
    let badge = node
        .props
        .badge
        .as_deref()
        .filter(|badge| !badge.is_empty())?;
    let badge_w = badge_width_for_text(&node.style, badge, theme, sf);
    let badge_h = badge_height_for_style(&node.style, theme, sf).min((rect.h - 4.0 * sf).max(1.0));
    let x = rect.x + rect.w - right_inset - badge_w;
    let y = rect.y + (rect.h - badge_h) * 0.5;
    if x <= rect.x || badge_w <= 0.0 || badge_h <= 0.0 {
        return None;
    }
    Some(Rect {
        x,
        y,
        w: badge_w,
        h: badge_h,
    })
}

fn badge_reserved_width(node: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
    node.props
        .badge
        .as_deref()
        .filter(|badge| !badge.is_empty())
        .map(|badge| badge_width_for_text(&node.style, badge, theme, sf) + BADGE_GAP_LP * sf)
        .unwrap_or(0.0)
}

fn contrast_glyph_color(bg: [f32; 4]) -> Color {
    let luminance = 0.2126 * bg[0] + 0.7152 * bg[1] + 0.0722 * bg[2];
    if luminance > 0.55 {
        Color::rgba(20, 20, 24, 255)
    } else {
        Color::rgba(255, 255, 255, 255)
    }
}

fn collect_dropdown_overlay_text(
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    if node.kind == WidgetKind::Dropdown && state.open_dropdown.as_deref() == Some(node.id.as_str())
    {
        if let (Some(r), Some(items)) = (
            layout.rects.get(&node.id),
            state.dropdown_items.get(&node.id),
        ) {
            let font_size = text_font_size(node, theme, sf);
            let line_height = text_line_height_for_style(&node.style.text, font_size, theme, sf);
            let font_family = node.style.text.font_family.as_ref();
            let font_weight = node.style.text.font_weight.unwrap_or(Weight::NORMAL.0);
            let text_options = text_options_from_style(&node.style.text);
            let row_h = theme.control_height() * sf;
            let selected = state.dropdown_index.get(&node.id).copied().unwrap_or(0);
            let hovered = state
                .dropdown_hover
                .as_ref()
                .filter(|(id, _)| id == &node.id)
                .map(|(_, idx)| *idx);
            for (idx, item) in items.iter().enumerate() {
                let y = r.y + r.h + idx as f32 * row_h;
                let part_names: &[&str] = if Some(idx) == hovered && idx == selected {
                    &["item-hover", "item-selected", "item"]
                } else if Some(idx) == hovered {
                    &["item-hover", "item"]
                } else if idx == selected {
                    &["item-selected", "item"]
                } else {
                    &["item"]
                };
                let item_pad = part_padding(node, part_names, pad, sf);
                let color =
                    part_text_color(node, state, theme, part_names, glyph_color(theme.text));
                push_text_entry(
                    font_system,
                    font_aliases,
                    out,
                    item,
                    font_size,
                    line_height,
                    font_family,
                    font_weight,
                    r.x + item_pad,
                    (y + ((row_h - line_height) * 0.5).max(0.0)).round(),
                    TextBounds {
                        left: (r.x + item_pad) as i32,
                        top: y as i32,
                        right: (r.x + r.w - item_pad) as i32,
                        bottom: (y + row_h) as i32,
                    },
                    color,
                    TextAlign::Left,
                    cache,
                    None,
                    caret_positions,
                    text_options,
                );
            }
        }
    }

    visit_stacking_children(node, |child| {
        collect_dropdown_overlay_text(
            child,
            layout,
            state,
            theme,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    });
}

fn collect_menu_overlay_text(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    if let Some(menu_id) = state.open_menu.as_deref() {
        collect_single_menu_overlay_text(
            tree,
            layout,
            state,
            theme,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
            menu_id,
        );
    }
    if let Some(menu_id) = state.open_context_menu.as_deref() {
        collect_single_menu_overlay_text(
            tree,
            layout,
            state,
            theme,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
            menu_id,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_single_menu_overlay_text(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
    menu_id: &str,
) {
    let Some(rect) = menu_popup_rect(tree, layout, state, theme, sf, menu_id) else {
        return;
    };
    let Some(items) = state.menu_items.get(menu_id) else {
        return;
    };
    let Some(node) = find_node(tree, menu_id) else {
        return;
    };
    let base_font_size = text_font_size(node, theme, sf);
    let font_size = part_font_size(node, &["item"], base_font_size, sf);
    let line_height = text_line_height_for_parts(node, &["item"], font_size, theme, sf);
    let font_family = part_font_family(node, &["item"], node.style.text.font_family.as_ref());
    let font_weight = part_font_weight(
        node,
        &["item"],
        node.style.text.font_weight.unwrap_or(Weight::NORMAL.0),
    );
    let text_options = text_options_for_parts(node, &["item"]);
    let row_h = theme.control_height() * sf;
    for (idx, item) in items.iter().enumerate() {
        let y = rect.y + idx as f32 * row_h;
        let disabled = item.disabled || state.is_disabled(&item.id);
        let hovered = state.hovered.as_deref() == Some(item.id.as_str());
        let parts: &[&str] = if disabled {
            &["item-disabled", "item"]
        } else if hovered {
            &["item-hover", "item"]
        } else {
            &["item"]
        };
        let color = part_text_color(
            node,
            state,
            theme,
            parts,
            glyph_color(if disabled {
                theme.muted_text
            } else {
                theme.text
            }),
        );
        push_text_entry(
            font_system,
            font_aliases,
            out,
            &item.value,
            font_size,
            line_height,
            font_family,
            font_weight,
            rect.x + pad,
            y + ((row_h - line_height) * 0.5).max(0.0),
            TextBounds {
                left: (rect.x + pad) as i32,
                top: y as i32,
                right: (rect.x + rect.w - pad) as i32,
                bottom: (y + row_h) as i32,
            },
            color,
            TextAlign::Left,
            cache,
            None,
            caret_positions,
            text_options,
        );
    }
}

fn collect_tooltip_text(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    stylesheets: &StylesheetStore,
    media: DgMediaEnvironment,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    if rich_tooltip_target(tree, layout, state).is_some() {
        return;
    }
    let Some((node, rect)) = tooltip_target(tree, layout, theme, state, sf) else {
        return;
    };
    let Some(text) = node.props.tooltip.as_deref() else {
        return;
    };
    let style = computed_style_for_virtual_element_with_media(
        WidgetKind::Tooltip,
        "__dg_static_tooltip",
        &["static"],
        stylesheets,
        Some(media),
    );
    let pad = uniform_layout_padding(&style.layout)
        .map(|padding| padding.max(0.0) * sf)
        .unwrap_or(theme.spacing * sf * 1.25);
    let font_size = style
        .text
        .font_size
        .map(|font_size| font_size.max(1.0) * sf)
        .unwrap_or_else(|| (theme.font_size * sf).max(8.0 * sf));
    let line_height = text_line_height_for_style(&style.text, font_size, theme, sf);
    let top = rect.y + pad;
    let opacity = overlay_opacity(&style, 1.0);
    push_wrapped_text_entry(
        font_system,
        font_aliases,
        out,
        text,
        font_size,
        line_height,
        style.text.font_family.as_ref(),
        style.text.font_weight.unwrap_or(Weight::NORMAL.0),
        rect.x + pad,
        top,
        TextBounds {
            left: (rect.x + pad) as i32,
            top: rect.y as i32,
            right: (rect.x + rect.w - pad) as i32,
            bottom: (rect.y + rect.h) as i32,
        },
        overlay_text_color(&style, theme, theme.text, opacity),
        style.text.text_align.unwrap_or(TextAlign::Left),
        cache,
        caret_positions,
        text_options_from_style(&style.text),
    );
}

#[allow(clippy::too_many_arguments)]
fn collect_rich_tooltip_text(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    let Some((node, _rect)) = rich_tooltip_target(tree, layout, state) else {
        return;
    };
    visit_stacking_children(node, |child| {
        collect_text(
            child,
            layout,
            state,
            theme,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            None,
            &[],
            false,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    });
}

fn collect_toast_text(
    toasts: &[ToastOverlay],
    theme: &Theme,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    window_w: f32,
    window_h: f32,
    stylesheets: &StylesheetStore,
    media: DgMediaEnvironment,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
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
        let font_size = style
            .text
            .font_size
            .map(|font_size| font_size.max(1.0) * sf)
            .unwrap_or_else(|| (theme.font_size * sf).max(8.0 * sf));
        let line_height = text_line_height_for_style(&style.text, font_size, theme, sf);
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
        );
        let pad = toast_padding(padding, sf);
        if rect.w <= pad * 2.0 || rect.h <= pad * 2.0 {
            continue;
        }
        let colors = toast_colors(toast.level, theme, 1.0);
        let opacity = overlay_opacity(&style, toast.opacity);
        push_wrapped_text_entry(
            font_system,
            font_aliases,
            out,
            &toast.message,
            font_size,
            line_height,
            style.text.font_family.as_ref(),
            style.text.font_weight.unwrap_or(Weight::MEDIUM.0),
            rect.x + pad,
            rect.y + ((rect.h - line_height) * 0.5).max(pad * 0.5),
            TextBounds {
                left: (rect.x + pad) as i32,
                top: rect.y as i32,
                right: (rect.x + rect.w - pad) as i32,
                bottom: (rect.y + rect.h) as i32,
            },
            overlay_text_color(&style, theme, colors.text, opacity),
            style.text.text_align.unwrap_or(TextAlign::Left),
            cache,
            caret_positions,
            text_options_from_style(&style.text),
        );
    }
}

fn collect_table_text(
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    resources: &ResourceRegistry,
    theme: &Theme,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
    extra_overlays: &[Rect],
    skip_open_modals: bool,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
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
    let subtree_text_start = out.len();
    if node.kind == WidgetKind::DataFrameTable {
        if let (Some(r), Some(_visible), Some(table_state)) = (
            layout.rects.get(&node.id).copied(),
            layout.visible_rect(&node.id),
            state.table(&node.id),
        ) {
            if r.w > 0.0 && r.h > 0.0 {
                let font_size = text_font_size(node, theme, sf);
                let font_family = node.style.text.font_family.as_ref();
                let font_weight = node.style.text.font_weight.unwrap_or(Weight::NORMAL.0);
                let metrics = table::metrics_for_node(node, theme, sf);
                let visible = table::visible(table_state, &r, metrics);
                let table_text_color = text_color(node, state, theme, false);
                let muted = glyph_color(theme.muted_text);
                let table_bottom = r.y + r.h;
                let header_bottom = (r.y + metrics.header_h).min(table_bottom);
                let table_radii = table_text_radii(node, theme, sf);
                let header_parts = &["header"];
                let header_font_size = part_font_size(node, header_parts, font_size, sf);
                let header_line_height =
                    text_line_height_for_parts(node, header_parts, header_font_size, theme, sf);
                let header_font_family = part_font_family(node, header_parts, font_family);
                let header_font_weight = part_font_weight(node, header_parts, font_weight);
                let header_text_options = text_options_for_parts(node, header_parts);
                let header_color =
                    part_text_color(node, state, theme, header_parts, table_text_color);
                let header_muted = part_text_color(node, state, theme, header_parts, muted);
                let mut index_header_bounds = table_text_bounds(
                    r,
                    Rect {
                        x: r.x,
                        y: r.y,
                        w: metrics.index_w,
                        h: header_h_for_bounds(metrics.header_h, r.h),
                    },
                    pad,
                    table_radii,
                );
                if table_state
                    .sort
                    .is_some_and(|(target, _)| target == TableSortColumn::Index)
                {
                    let reserve = (DROPDOWN_CHEVRON_WIDTH_LP * sf + pad).ceil() as i32;
                    index_header_bounds.right =
                        (index_header_bounds.right - reserve).max(index_header_bounds.left + 1);
                }
                let index_header_bounds =
                    clamp_text_bounds_bottom(index_header_bounds, header_bottom);

                if !text_bounds_obscured_by_overlay(
                    node,
                    index_header_bounds,
                    open_dropdown,
                    dropdown_overlay,
                    menu_overlays,
                    tooltip_overlay,
                    extra_overlays,
                ) {
                    push_text_entry(
                        font_system,
                        font_aliases,
                        out,
                        "#",
                        header_font_size,
                        header_line_height,
                        header_font_family,
                        header_font_weight,
                        index_header_bounds.left as f32,
                        r.y + ((metrics.header_h - header_line_height) * 0.5).max(0.0),
                        index_header_bounds,
                        header_muted,
                        TextAlign::Left,
                        cache,
                        None,
                        caret_positions,
                        header_text_options,
                    );
                }

                for col_offset in 0..visible.col_count {
                    let col = visible.first_col + col_offset;
                    let Some((col_x, col_right)) =
                        table::column_bounds(table_state, &r, metrics, col_offset)
                    else {
                        continue;
                    };
                    let name = table_state
                        .columns
                        .get(col)
                        .map(String::as_str)
                        .unwrap_or("");
                    let mut bounds = table_text_bounds(
                        r,
                        Rect {
                            x: col_x,
                            y: r.y,
                            w: col_right - col_x,
                            h: header_h_for_bounds(metrics.header_h, r.h),
                        },
                        pad,
                        table_radii,
                    );
                    if table_state
                        .sort
                        .is_some_and(|(target, _)| target == TableSortColumn::Data(col))
                    {
                        let reserve = (DROPDOWN_CHEVRON_WIDTH_LP * sf + pad).ceil() as i32;
                        bounds.right = (bounds.right - reserve).max(bounds.left + 1);
                    }
                    let bounds = clamp_text_bounds_bottom(bounds, header_bottom);
                    if !text_bounds_obscured_by_overlay(
                        node,
                        bounds,
                        open_dropdown,
                        dropdown_overlay,
                        menu_overlays,
                        tooltip_overlay,
                        extra_overlays,
                    ) {
                        push_text_entry(
                            font_system,
                            font_aliases,
                            out,
                            name,
                            header_font_size,
                            header_line_height,
                            header_font_family,
                            header_font_weight,
                            bounds.left as f32,
                            r.y + ((metrics.header_h - header_line_height) * 0.5).max(0.0),
                            bounds,
                            header_color,
                            TextAlign::Left,
                            cache,
                            None,
                            caret_positions,
                            header_text_options,
                        );
                    }
                }

                for row_offset in 0..visible.row_count {
                    let row = visible.first_row + row_offset;
                    let Some((row_y, row_bottom)) = table::row_bounds(&r, metrics, row_offset)
                    else {
                        continue;
                    };

                    let selected = table_state
                        .selected
                        .is_some_and(|(selected_row, _)| selected_row == row);
                    let row_parts: &[&str] = if selected {
                        &["row-selected", "row"]
                    } else {
                        &["row"]
                    };
                    let row_font_size = part_font_size(node, row_parts, font_size, sf);
                    let row_line_height =
                        text_line_height_for_parts(node, row_parts, row_font_size, theme, sf);
                    let row_font_family = part_font_family(node, row_parts, font_family);
                    let row_font_weight = part_font_weight(node, row_parts, font_weight);
                    let row_text_options = text_options_for_parts(node, row_parts);
                    let row_text_color =
                        part_text_color(node, state, theme, row_parts, table_text_color);
                    let row_muted = part_text_color(node, state, theme, row_parts, muted);
                    let row_index_bounds = table_text_bounds(
                        r,
                        Rect {
                            x: r.x,
                            y: row_y,
                            w: metrics.index_w,
                            h: row_bottom - row_y,
                        },
                        pad,
                        table_radii,
                    );

                    if !text_bounds_obscured_by_overlay(
                        node,
                        row_index_bounds,
                        open_dropdown,
                        dropdown_overlay,
                        menu_overlays,
                        tooltip_overlay,
                        extra_overlays,
                    ) {
                        push_text_entry(
                            font_system,
                            font_aliases,
                            out,
                            &table::source_row(table_state, row).to_string(),
                            row_font_size,
                            row_line_height,
                            row_font_family,
                            row_font_weight,
                            row_index_bounds.left as f32,
                            row_y + ((metrics.row_h - row_line_height) * 0.5).max(0.0),
                            row_index_bounds,
                            row_muted,
                            TextAlign::Left,
                            cache,
                            None,
                            caret_positions,
                            row_text_options,
                        );
                    }

                    for col_offset in 0..visible.col_count {
                        let col = visible.first_col + col_offset;
                        let Some((col_x, col_right)) =
                            table::column_bounds(table_state, &r, metrics, col_offset)
                        else {
                            continue;
                        };
                        let value = table::cell_text(table_state, resources, row, col);
                        let bounds = table_text_bounds(
                            r,
                            Rect {
                                x: col_x,
                                y: row_y,
                                w: col_right - col_x,
                                h: row_bottom - row_y,
                            },
                            pad,
                            table_radii,
                        );
                        if !text_bounds_obscured_by_overlay(
                            node,
                            bounds,
                            open_dropdown,
                            dropdown_overlay,
                            menu_overlays,
                            tooltip_overlay,
                            extra_overlays,
                        ) {
                            push_text_entry(
                                font_system,
                                font_aliases,
                                out,
                                &value,
                                row_font_size,
                                row_line_height,
                                row_font_family,
                                row_font_weight,
                                bounds.left as f32,
                                row_y + ((metrics.row_h - row_line_height) * 0.5).max(0.0),
                                bounds,
                                row_text_color,
                                TextAlign::Left,
                                cache,
                                None,
                                caret_positions,
                                row_text_options,
                            );
                        }
                    }
                }
            }
        }
    }

    visit_stacking_children(node, |child| {
        collect_table_text(
            child,
            layout,
            state,
            resources,
            theme,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            extra_overlays,
            skip_open_modals,
            font_system,
            font_aliases,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    });
    if let Some(r) = layout.rects.get(&node.id) {
        apply_transform_to_text_entries(
            &mut out[subtree_text_start..],
            visual_transform_for_text(node, state),
            sf,
            [r.x + r.w * 0.5, r.y + r.h * 0.5],
        );
    }
    apply_paint_clip_to_text_entries(
        &mut out[subtree_text_start..],
        layout.paint_clip_rect(&node.id),
    );
}

fn table_text_radii(node: &WidgetNode, theme: &Theme, sf: f32) -> [f32; 4] {
    let radius = node
        .style
        .visual
        .border_radius
        .unwrap_or(theme.radius)
        .max(0.0);
    node.style
        .visual
        .corner_radii
        .resolve(radius)
        .map(|radius| radius.max(0.0) * sf)
}

fn header_h_for_bounds(header_h: f32, table_h: f32) -> f32 {
    header_h.min(table_h).max(0.0)
}

fn clamp_text_bounds_bottom(mut bounds: TextBounds, bottom: f32) -> TextBounds {
    bounds.bottom = bounds.bottom.min(bottom as i32);
    bounds
}

fn table_text_bounds(table: Rect, cell: Rect, pad: f32, radii: [f32; 4]) -> TextBounds {
    let table_right = table.x + table.w;
    let table_bottom = table.y + table.h;
    let cell_right = cell.x + cell.w;
    let cell_bottom = cell.y + cell.h;
    let max_radius = table.w.min(table.h) * 0.5;
    let [tl, tr, br, bl] = radii.map(|radius| radius.min(max_radius).max(0.0));

    let mut left = (cell.x + pad).max(table.x);
    let mut right = (cell_right - pad).min(table_right);
    let top = cell.y.max(table.y);
    let bottom = cell_bottom.min(table_bottom);

    let touches_left = cell.x <= table.x + 0.5;
    let touches_right = cell_right >= table_right - 0.5;
    if touches_left && cell.y < table.y + tl {
        left = left.max(table.x + tl);
    }
    if touches_left && cell_bottom > table_bottom - bl {
        left = left.max(table.x + bl);
    }
    if touches_right && cell.y < table.y + tr {
        right = right.min(table_right - tr);
    }
    if touches_right && cell_bottom > table_bottom - br {
        right = right.min(table_right - br);
    }
    if right <= left {
        right = (left + 1.0).min(table_right);
    }

    TextBounds {
        left: left.ceil() as i32,
        top: top.ceil() as i32,
        right: right.floor() as i32,
        bottom: bottom.floor() as i32,
    }
}

fn display_text<'a>(node: &'a WidgetNode, state: &'a WidgetState) -> Option<(&'a str, bool)> {
    match node.kind {
        WidgetKind::TextInput
        | WidgetKind::TextArea
        | WidgetKind::CodeEditor
        | WidgetKind::LogView
        | WidgetKind::NumberInput
        | WidgetKind::DragNumber => {
            let value = state.text_for(&node.id).unwrap_or("");
            if value.is_empty() {
                state
                    .placeholder_for(&node.id)
                    .filter(|p| !p.is_empty())
                    .map(|p| (p, true))
            } else {
                Some((value, false))
            }
        }
        WidgetKind::Dropdown => state
            .dropdown_value(&node.id)
            .filter(|t| !t.is_empty())
            .map(|t| (t, false)),
        WidgetKind::IconButton => icon_button_symbol_text(node).map(|text| (text, false)),
        _ => node
            .props
            .text
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(|t| (t, false)),
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_log_view_lines(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    rect: Rect,
    text: &str,
    scroll_y: f32,
    sf: f32,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
    font_weight: u16,
    text_bounds: TextBounds,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    if line_height <= 0.0 || text_bounds.right <= text_bounds.left {
        return;
    }
    let bottom_bleed = (line_height * 0.30)
        .max(2.0 * sf)
        .min((rect.h * 0.08).max(0.0));
    let bounds = TextBounds {
        bottom: ((text_bounds.bottom as f32 + bottom_bleed).min(rect.y + rect.h)) as i32,
        ..text_bounds
    };
    let total_lines = text.split('\n').count().max(1);
    let first = (scroll_y / line_height).floor().max(0.0) as usize;
    let visible_h = (bounds.bottom - bounds.top).max(1) as f32;
    let count = ((visible_h / line_height).ceil() as usize).saturating_add(2);
    let last = first.saturating_add(count).min(total_lines);
    if first >= last {
        return;
    }
    let line_font_size = part_font_size(node, &["line"], font_size, sf);
    let line_font_family = part_font_family(node, &["line"], font_family);
    let line_font_weight = part_font_weight(node, &["line"], font_weight);
    let base_color = part_text_color(
        node,
        state,
        theme,
        &["line"],
        text_color(node, state, theme, false),
    );
    let line_options = text_options_from_styles(part_text_style(node, &["line"]), &node.style.text);
    let left = bounds.left as f32;
    let top_base = text_bounds.top as f32 - scroll_y;
    for (index, line) in text.split('\n').enumerate().skip(first).take(last - first) {
        let top = top_base + index as f32 * line_height;
        if top + line_height < bounds.top as f32 || top > bounds.bottom as f32 {
            continue;
        }
        let part = log_line_level_part(line);
        let color = if part == "line" {
            base_color
        } else {
            part_text_color(node, state, theme, &[part, "line"], base_color)
        };
        let options = text_options_from_styles(
            part_text_style(node, &[part, "line"]),
            part_text_style(node, &["line"]).unwrap_or(&node.style.text),
        );
        push_text_entry(
            font_system,
            font_aliases,
            out,
            line,
            line_font_size,
            line_height,
            line_font_family,
            line_font_weight,
            left,
            top,
            bounds,
            color,
            TextAlign::Left,
            cache,
            None,
            caret_positions,
            if part == "line" {
                line_options
            } else {
                options
            },
        );
    }
}

fn log_line_level_part(line: &str) -> &'static str {
    let lower = line.trim_start().to_ascii_lowercase();
    if lower.contains("error") || lower.contains("fatal") || lower.contains("[err]") {
        "error"
    } else if lower.contains("warn") {
        "warning"
    } else if lower.contains("debug") || lower.contains("trace") {
        "debug"
    } else if lower.contains("info") {
        "info"
    } else {
        "line"
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_code_editor_line_numbers(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    rect: Rect,
    pad: f32,
    sf: f32,
    text: &str,
    line_height: f32,
    font_size: f32,
    font_family: Option<&FontFamily>,
    font_weight: u16,
    scroll_y: f32,
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    let gutter_w = code_editor_gutter_width_for_style(&node.style, sf)
        .min((rect.w - pad * 2.0).max(1.0) * 0.5);
    if gutter_w <= 1.0 || line_height <= 0.0 {
        return;
    }
    let left_inset = (pad * 0.5).max(4.0 * sf);
    let right_inset = part_padding(node, &["line-number"], pad, sf).max(6.0 * sf);
    let bottom_bleed = (line_height * 0.30).max(2.0 * sf).min(pad);
    let clip_rect = Rect {
        x: rect.x + left_inset,
        y: rect.y + pad,
        w: (gutter_w - left_inset - right_inset).max(1.0),
        h: (rect.h - pad * 2.0 + bottom_bleed).max(1.0),
    }
    .intersect(rect);
    let Some(clip_rect) = clip_rect else {
        return;
    };
    let bounds = TextBounds {
        left: clip_rect.x as i32,
        top: clip_rect.y as i32,
        right: (clip_rect.x + clip_rect.w) as i32,
        bottom: (clip_rect.y + clip_rect.h) as i32,
    };
    let total_lines = text.split('\n').count().max(1);
    let first = (scroll_y / line_height).floor().max(0.0) as usize;
    let count = ((clip_rect.h / line_height).ceil() as usize).saturating_add(2);
    let last = first.saturating_add(count).min(total_lines);
    if first >= last {
        return;
    }
    let line_font_size = part_font_size(node, &["line-number"], font_size, sf);
    let line_font_family = part_font_family(node, &["line-number"], font_family);
    let line_font_weight = part_font_weight(node, &["line-number"], font_weight);
    let line_color = part_text_color(
        node,
        state,
        theme,
        &["line-number"],
        glyph_color(theme.muted_text),
    );
    let options =
        text_options_from_styles(part_text_style(node, &["line-number"]), &node.style.text);
    let left = rect.x + left_inset;
    let top_base = rect.y + pad - scroll_y;
    for index in first..last {
        let top = top_base + index as f32 * line_height;
        if top + line_height < clip_rect.y || top > clip_rect.y + clip_rect.h {
            continue;
        }
        let label = (index + 1).to_string();
        push_text_entry(
            font_system,
            font_aliases,
            out,
            &label,
            line_font_size,
            line_height,
            line_font_family,
            line_font_weight,
            left,
            top,
            bounds,
            line_color,
            TextAlign::Right,
            cache,
            None,
            caret_positions,
            options,
        );
    }
}

fn push_text_entry(
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    out: &mut Vec<TextEntry>,
    text: &str,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
    font_weight: u16,
    left: f32,
    top: f32,
    clip: TextBounds,
    color: Color,
    align: TextAlign,
    cache: &mut TextBufferCache,
    caret: Option<(&str, usize)>,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    options: TextRenderOptions,
) {
    push_text_entry_impl(
        font_system,
        font_aliases,
        out,
        text,
        font_size,
        line_height,
        font_family,
        font_weight,
        left,
        top,
        clip,
        color,
        align,
        false,
        0.0,
        0.0,
        cache,
        caret,
        caret_positions,
        options,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_wrapped_text_entry(
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    out: &mut Vec<TextEntry>,
    text: &str,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
    font_weight: u16,
    left: f32,
    top: f32,
    clip: TextBounds,
    color: Color,
    align: TextAlign,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    options: TextRenderOptions,
) {
    push_text_entry_impl(
        font_system,
        font_aliases,
        out,
        text,
        font_size,
        line_height,
        font_family,
        font_weight,
        left,
        top,
        clip,
        color,
        align,
        true,
        0.0,
        0.0,
        cache,
        None,
        caret_positions,
        options,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_text_entry_impl(
    font_system: &mut FontSystem,
    font_aliases: &FontFamilyAliases,
    out: &mut Vec<TextEntry>,
    text: &str,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
    font_weight: u16,
    left: f32,
    top: f32,
    clip: TextBounds,
    color: Color,
    align: TextAlign,
    wrap: bool,
    caret_scroll_x: f32,
    caret_scroll_y: f32,
    cache: &mut TextBufferCache,
    caret: Option<(&str, usize)>,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    options: TextRenderOptions,
) {
    if clip.right <= clip.left
        || clip.bottom <= clip.top
        || left >= clip.right as f32
        || top >= clip.bottom as f32
    {
        return;
    }

    let avail_w = (clip.right as f32 - left).max(1.0);
    let display_text = prepare_display_text(
        font_system,
        text,
        font_size,
        line_height,
        font_family,
        font_aliases,
        font_weight,
        avail_w,
        wrap,
        caret.is_some(),
        options,
    );
    let text = display_text.as_ref();
    let font_style = options.font_style.unwrap_or(FontStyle::Normal);
    let tabular_nums = options.font_variant_numeric == Some(FontVariantNumeric::TabularNums);
    let letter_spacing = resolved_letter_spacing_em(options.letter_spacing, font_size);
    let key = TextKey {
        text: text.to_string(),
        font_family: font_family_key(font_family, font_aliases),
        font_weight,
        font_style,
        tabular_nums,
        letter_spacing_milli: (letter_spacing.unwrap_or(0.0) * 1000.0).round() as i32,
        font_size_milli: (font_size * 1000.0).round() as i32,
        line_height_milli: (line_height * 1000.0).round() as i32,
        width_milli: (avail_w * 1000.0).round() as i32,
        wrap,
    };
    let buf = cache
        .get_mut(&key)
        .and_then(|buffers| buffers.pop())
        .unwrap_or_else(|| {
            let mut buf = Buffer::new(font_system, Metrics::new(font_size, line_height));
            buf.set_size(font_system, Some(avail_w), None);
            buf.set_wrap(font_system, if wrap { Wrap::Word } else { Wrap::None });
            let attrs = text_attrs(font_family, font_aliases, font_weight, options, font_size);
            buf.set_text(font_system, text, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(font_system, false);
            buf
        });
    if let Some((id, cursor)) = caret {
        let mut xy = caret_xy_for_buffer(&buf, text, cursor);
        xy[0] -= caret_scroll_x;
        xy[1] -= caret_scroll_y;
        caret_positions.insert(id.to_string(), xy);
    }
    let aligned_left = match align {
        TextAlign::Left => left,
        TextAlign::Center => {
            let text_w = text_width_for_buffer(&buf);
            let bounds_w = (clip.right - clip.left).max(1) as f32;
            clip.left as f32 + ((bounds_w - text_w).max(0.0) * 0.5)
        }
        TextAlign::Right => {
            let text_w = text_width_for_buffer(&buf);
            let bounds_w = (clip.right - clip.left).max(1) as f32;
            clip.left as f32 + (bounds_w - text_w).max(0.0)
        }
    };

    out.push(TextEntry {
        key,
        buffer: buf,
        left: aligned_left,
        top,
        scale: 1.0,
        clip,
        untransformed_clip: clip,
        color,
        custom_glyphs: Vec::new(),
    });
}

#[allow(clippy::too_many_arguments)]
fn prepare_display_text<'a>(
    font_system: &mut FontSystem,
    text: &'a str,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
    font_aliases: &FontFamilyAliases,
    font_weight: u16,
    avail_w: f32,
    wrap: bool,
    has_caret: bool,
    options: TextRenderOptions,
) -> Cow<'a, str> {
    let transformed = if has_caret {
        Cow::Borrowed(text)
    } else {
        transform_text(text, options.text_transform)
    };
    if wrap || has_caret || options.text_overflow != Some(TextOverflow::Ellipsis) {
        return transformed;
    }
    if text_width_for_measurement(
        font_system,
        transformed.as_ref(),
        font_size,
        line_height,
        font_family,
        font_aliases,
        font_weight,
        options,
    ) <= avail_w
    {
        return transformed;
    }
    Cow::Owned(ellipsize_text(
        font_system,
        transformed.as_ref(),
        font_size,
        line_height,
        font_family,
        font_aliases,
        font_weight,
        options,
        avail_w,
    ))
}

fn transform_text(text: &str, transform: Option<TextTransform>) -> Cow<'_, str> {
    match transform.unwrap_or(TextTransform::None) {
        TextTransform::None => Cow::Borrowed(text),
        TextTransform::Uppercase => Cow::Owned(text.to_uppercase()),
        TextTransform::Lowercase => Cow::Owned(text.to_lowercase()),
        TextTransform::Capitalize => Cow::Owned(capitalize_text(text)),
    }
}

fn capitalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word_start = true;
    for ch in text.chars() {
        if ch.is_whitespace() {
            word_start = true;
            out.push(ch);
        } else if word_start {
            out.extend(ch.to_uppercase());
            word_start = false;
        } else {
            out.push(ch);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn ellipsize_text(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
    font_aliases: &FontFamilyAliases,
    font_weight: u16,
    options: TextRenderOptions,
    avail_w: f32,
) -> String {
    const ELLIPSIS: &str = "...";
    if text_width_for_measurement(
        font_system,
        ELLIPSIS,
        font_size,
        line_height,
        font_family,
        font_aliases,
        font_weight,
        options,
    ) > avail_w
    {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut low = 0usize;
    let mut high = chars.len();
    while low < high {
        let mid = (low + high).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>() + ELLIPSIS;
        if text_width_for_measurement(
            font_system,
            &candidate,
            font_size,
            line_height,
            font_family,
            font_aliases,
            font_weight,
            options,
        ) <= avail_w
        {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    chars[..low].iter().collect::<String>() + ELLIPSIS
}

#[allow(clippy::too_many_arguments)]
fn text_width_for_measurement(
    font_system: &mut FontSystem,
    text: &str,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
    font_aliases: &FontFamilyAliases,
    font_weight: u16,
    options: TextRenderOptions,
) -> f32 {
    let mut buf = Buffer::new(font_system, Metrics::new(font_size, line_height));
    buf.set_size(font_system, Some(1_000_000.0), None);
    buf.set_wrap(font_system, Wrap::None);
    let attrs = text_attrs(font_family, font_aliases, font_weight, options, font_size);
    buf.set_text(font_system, text, &attrs, Shaping::Advanced, None);
    buf.shape_until_scroll(font_system, false);
    text_width_for_buffer(&buf)
}

fn text_attrs<'a>(
    font_family: Option<&'a FontFamily>,
    font_aliases: &'a FontFamilyAliases,
    font_weight: u16,
    options: TextRenderOptions,
    font_size: f32,
) -> Attrs<'a> {
    let glyph_style = match options.font_style.unwrap_or(FontStyle::Normal) {
        FontStyle::Normal => GlyphStyle::Normal,
        FontStyle::Italic => GlyphStyle::Italic,
    };
    let mut attrs = Attrs::new()
        .family(to_glyphon_family(font_family, font_aliases))
        .weight(Weight(font_weight))
        .style(glyph_style);
    if let Some(spacing) = resolved_letter_spacing_em(options.letter_spacing, font_size) {
        attrs = attrs.letter_spacing(spacing);
    }
    if options.font_variant_numeric == Some(FontVariantNumeric::TabularNums) {
        let mut features = FontFeatures::new();
        features.enable(FeatureTag::new(b"tnum"));
        attrs = attrs.font_features(features);
    }
    attrs
}

fn resolved_letter_spacing_em(spacing: Option<TextSpacing>, font_size: f32) -> Option<f32> {
    match spacing {
        Some(TextSpacing::LogicalPx(px)) if font_size > 0.0 => Some(px / font_size),
        Some(TextSpacing::Em(em)) => Some(em),
        _ => None,
    }
}

fn font_family_key(family: Option<&FontFamily>, aliases: &FontFamilyAliases) -> String {
    match family {
        Some(FontFamily::Serif) => "serif".to_string(),
        Some(FontFamily::SansSerif) | None => "sans-serif".to_string(),
        Some(FontFamily::Monospace) => "monospace".to_string(),
        Some(FontFamily::Cursive) => "cursive".to_string(),
        Some(FontFamily::Fantasy) => "fantasy".to_string(),
        Some(FontFamily::Name(name)) => {
            format!("name:{}", aliases.get(name).unwrap_or(name))
        }
    }
}

fn to_glyphon_family<'a>(
    family: Option<&'a FontFamily>,
    aliases: &'a FontFamilyAliases,
) -> Family<'a> {
    match family {
        Some(FontFamily::Serif) => Family::Serif,
        Some(FontFamily::SansSerif) | None => Family::SansSerif,
        Some(FontFamily::Monospace) => Family::Monospace,
        Some(FontFamily::Cursive) => Family::Cursive,
        Some(FontFamily::Fantasy) => Family::Fantasy,
        Some(FontFamily::Name(name)) => Family::Name(
            aliases
                .get(name)
                .map(String::as_str)
                .unwrap_or(name.as_str()),
        ),
    }
}

fn text_width_for_buffer(buffer: &Buffer) -> f32 {
    buffer
        .layout_runs()
        .find(|run| run.line_i == 0)
        .map(|run| run.line_w)
        .unwrap_or(0.0)
}

fn caret_xy_for_buffer(buffer: &Buffer, text: &str, cursor: usize) -> [f32; 2] {
    let cursor = clamp_boundary(text, cursor);
    if cursor == 0 || text.is_empty() {
        return [0.0, 0.0];
    }

    let mut last = [0.0, 0.0];
    for run in buffer.layout_runs() {
        let line_start = line_start_byte(text, run.line_i);
        let line_end = line_end_byte(text, line_start);
        if cursor < line_start || cursor > line_end {
            continue;
        }
        if cursor == line_start {
            return [0.0, run.line_top.max(0.0)];
        }
        for glyph in run.glyphs {
            let glyph_start = line_start + glyph.start;
            let glyph_end = line_start + glyph.end;
            let glyph_left = glyph.x;
            let glyph_right = glyph.x + glyph.w;
            last = [glyph_right.max(last[0]), run.line_top.max(0.0)];

            if cursor == glyph_start {
                return [glyph_left.max(0.0), run.line_top.max(0.0)];
            }
            if cursor == glyph_end {
                return [glyph_right.max(0.0), run.line_top.max(0.0)];
            }
            if cursor > glyph_start && cursor < glyph_end {
                let before = text[glyph_start..cursor].chars().count() as f32;
                let total = text[glyph_start..glyph_end].chars().count().max(1) as f32;
                let t = (before / total).clamp(0.0, 1.0);
                return [(glyph_left + glyph.w * t).max(0.0), run.line_top.max(0.0)];
            }
        }
        return last;
    }

    last
}

fn line_start_byte(text: &str, target_line: usize) -> usize {
    if target_line == 0 {
        return 0;
    }
    let mut line = 0;
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            line += 1;
            if line == target_line {
                return idx + ch.len_utf8();
            }
        }
    }
    text.len()
}

fn line_end_byte(text: &str, line_start: usize) -> usize {
    text[line_start..]
        .find('\n')
        .map(|idx| line_start + idx)
        .unwrap_or(text.len())
}

fn clamp_boundary(s: &str, idx: usize) -> usize {
    let mut clamped = idx.min(s.len());
    while clamped > 0 && !s.is_char_boundary(clamped) {
        clamped -= 1;
    }
    clamped
}

fn glyph_color(color: [f32; 4]) -> Color {
    Color::rgba(
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
        (color[3].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::NodeProps;
    use crate::events::{NavigationItem, TableState};
    use crate::resources::ResourceRegistry;

    fn node(id: &str, kind: WidgetKind) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name: None,
            kind,
            props: NodeProps::default(),
            style_json: Default::default(),
            inline_style: Default::default(),
            style: Default::default(),
            children: Vec::new(),
        }
    }

    fn env_usize(name: &str, default: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(default)
    }

    #[test]
    fn extension_display_list_text_emits_text_entry() {
        let mut extension = node("paint", WidgetKind::Extension);
        let props = serde_json::json!({
            "extension_type": "paint",
            "paint_width": 100,
            "paint_height": 50,
            "display_list": [
                {"cmd": "text", "x": 10, "y": 8, "text": "Loss", "fill": "accent", "font_size": 12, "font_weight": 700}
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
        let theme = Theme::dark();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut caret_positions = HashMap::new();
        let mut cache = TextBufferCache::default();
        let mut out = Vec::new();

        collect_text(
            &extension,
            &layout,
            &WidgetState::default(),
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            true,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut out,
        );

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].key.text, "Loss");
        assert_eq!(out[0].key.font_weight, 700);
        assert_eq!(out[0].left, 20.0);
        assert_eq!(out[0].top, 16.0);
        assert_eq!(out[0].key.font_size_milli, 24000);
        assert_eq!(out[0].color, glyph_color(theme.accent));
    }

    fn table_text_bench_fixture() -> (
        WidgetNode,
        LayoutResult,
        WidgetState,
        ResourceRegistry,
        usize,
    ) {
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
        let visible_cells = state
            .table("table")
            .map(|state| {
                let metrics = table::metrics_for_node(&table, &Theme::dark(), 1.0);
                let rect = layout.rects.get("table").copied().unwrap();
                let visible = table::visible(state, &rect, metrics);
                visible.row_count * visible.col_count
            })
            .unwrap_or(0);
        (
            table,
            layout,
            state,
            ResourceRegistry::default(),
            visible_cells,
        )
    }

    fn many_labels_text_bench_fixture(count: usize) -> (WidgetNode, LayoutResult) {
        let mut root = node("root", WidgetKind::FlowLayout);
        root.children.reserve(count);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            root.id.clone(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1200.0,
                h: (count as f32 * 22.0).max(1.0),
            },
        );
        for index in 0..count {
            let mut label = node(&format!("label-{index}"), WidgetKind::Label);
            label.props.text = Some(format!("Metric {index}: {}", index % 997));
            layout.rects.insert(
                label.id.clone(),
                Rect {
                    x: 0.0,
                    y: index as f32 * 22.0,
                    w: 360.0,
                    h: 20.0,
                },
            );
            root.children.push(label);
        }
        (root, layout)
    }

    #[test]
    #[ignore]
    fn bench_table_text_collect() {
        let iterations = env_usize("DRAGONGUI_TABLE_BENCH_TEXT_ITERS", 200);
        let warmup = env_usize("DRAGONGUI_TABLE_BENCH_TEXT_WARMUP", 20);
        let (tree, layout, state, resources, visible_cells) = table_text_bench_fixture();
        let theme = Theme::dark();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut caret_positions = HashMap::new();
        let mut cache = TextBufferCache::default();
        let mut out = Vec::new();

        for _ in 0..warmup {
            out.clear();
            collect_table_text(
                &tree,
                &layout,
                &state,
                &resources,
                &theme,
                None,
                None,
                [None, None],
                None,
                &[],
                true,
                &mut font_system,
                &font_aliases,
                1.0,
                theme.spacing,
                &mut cache,
                &mut caret_positions,
                &mut out,
            );
            for entry in out.drain(..) {
                cache.entry(entry.key).or_default().push(entry.buffer);
            }
        }

        let start = std::time::Instant::now();
        let mut emitted = 0usize;
        for _ in 0..iterations {
            collect_table_text(
                &tree,
                &layout,
                &state,
                &resources,
                &theme,
                None,
                None,
                [None, None],
                None,
                &[],
                true,
                &mut font_system,
                &font_aliases,
                1.0,
                theme.spacing,
                &mut cache,
                &mut caret_positions,
                &mut out,
            );
            emitted += out.len();
            for entry in out.drain(..) {
                cache.entry(entry.key).or_default().push(entry.buffer);
            }
        }
        let elapsed = start.elapsed();
        let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
        let ns_per_visible_cell = if visible_cells == 0 {
            0.0
        } else {
            elapsed.as_nanos() as f64 / (iterations * visible_cells) as f64
        };
        eprintln!(
            "table text collect: iterations={iterations} visible_cells={visible_cells} total_ms={:.3} ns_per_iter={:.1} ns_per_visible_cell={:.1} entries_per_iter={:.2}",
            elapsed.as_secs_f64() * 1000.0,
            ns_per_iter,
            ns_per_visible_cell,
            emitted as f64 / iterations as f64
        );
    }

    #[test]
    #[ignore]
    fn bench_text_collect_many_labels() {
        let count = env_usize("DRAGONGUI_BENCH_TEXT_LABELS", 2_000);
        let iterations = env_usize("DRAGONGUI_BENCH_TEXT_ITERS", 200);
        let warmup = env_usize("DRAGONGUI_BENCH_TEXT_WARMUP", 20);
        let (tree, layout) = many_labels_text_bench_fixture(count);
        let state = WidgetState::from_tree(&tree);
        let theme = Theme::dark();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut caret_positions = HashMap::new();
        let mut cache = TextBufferCache::default();
        let mut out = Vec::new();

        for _ in 0..warmup {
            out.clear();
            collect_text(
                &tree,
                &layout,
                &state,
                &theme,
                None,
                None,
                [None, None],
                None,
                &[],
                true,
                &mut font_system,
                &font_aliases,
                1.0,
                theme.spacing,
                &mut cache,
                &mut caret_positions,
                &mut out,
            );
            for entry in out.drain(..) {
                cache.entry(entry.key).or_default().push(entry.buffer);
            }
        }

        let start = std::time::Instant::now();
        let mut emitted = 0usize;
        for _ in 0..iterations {
            collect_text(
                &tree,
                &layout,
                &state,
                &theme,
                None,
                None,
                [None, None],
                None,
                &[],
                true,
                &mut font_system,
                &font_aliases,
                1.0,
                theme.spacing,
                &mut cache,
                &mut caret_positions,
                &mut out,
            );
            emitted += out.len();
            for entry in out.drain(..) {
                cache.entry(entry.key).or_default().push(entry.buffer);
            }
        }
        let elapsed = start.elapsed();
        eprintln!(
            "text collect many labels: labels={count} iterations={iterations} total_ms={:.3} ns_per_label={:.1} entries_per_iter={:.1}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / (iterations * count) as f64,
            emitted as f64 / iterations as f64
        );
    }

    #[test]
    fn generated_content_attr_reads_widget_props() {
        let mut badge = node("state", WidgetKind::Badge);
        badge.props.raw_props.insert(
            "level".to_string(),
            serde_json::Value::String("success".to_string()),
        );
        badge.class_name = Some("metric".to_string());

        assert_eq!(
            generated_content_text(&badge, &GeneratedContent::Attr("level".to_string())).as_deref(),
            Some("success")
        );
        assert_eq!(
            generated_content_text(&badge, &GeneratedContent::Attr("class".to_string())).as_deref(),
            Some("metric")
        );
    }

    #[test]
    fn panel_title_padding_uses_custom_style_padding() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.layout.padding = Some(16.0);
        panel.style.layout.padding_right = Some(20.0);

        let padding = panel_title_padding(&panel, &Theme::dark(), 1.0);

        assert_eq!(padding.left, 16.0);
        assert_eq!(padding.right, 20.0);
        assert_eq!(padding.top, 16.0);
    }

    #[test]
    fn table_text_bounds_avoid_rounded_corners() {
        let table = Rect {
            x: 10.0,
            y: 20.0,
            w: 200.0,
            h: 120.0,
        };
        let top_left_cell = Rect {
            x: 10.0,
            y: 20.0,
            w: 42.0,
            h: 30.0,
        };
        let bottom_right_cell = Rect {
            x: 168.0,
            y: 112.0,
            w: 42.0,
            h: 28.0,
        };

        let top_bounds = table_text_bounds(table, top_left_cell, 6.0, [18.0, 18.0, 18.0, 18.0]);
        let bottom_bounds =
            table_text_bounds(table, bottom_right_cell, 6.0, [18.0, 18.0, 18.0, 18.0]);

        assert_eq!(top_bounds.left, 28);
        assert_eq!(bottom_bounds.right, 192);
    }

    #[test]
    fn tab_text_is_vertically_centered_without_extra_top_offset() {
        let mut tab = node("debug", WidgetKind::Tab);
        tab.style.text.font_size = Some(14.0);
        tab.style.parts.parts.insert(
            "tab".to_string(),
            crate::style::PartStyle {
                layout: crate::style::PartLayoutStyle {
                    padding: Some(8.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let r = Rect {
            x: 0.0,
            y: 0.0,
            w: 90.0,
            h: 30.0,
        };
        let theme = Theme::dark();
        let font_size = text_font_size(&tab, &theme, 1.0);
        let line_height = text_line_height(font_size, &theme, 1.0);
        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);

        assert_eq!(top, 5.5);
    }

    #[test]
    fn button_text_defaults_to_center_alignment() {
        let mut button = node("edge", WidgetKind::Button);
        button.props.text = Some("Top left".to_string());
        button.style.text.font_size = Some(14.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "edge".to_string(),
            Rect {
                x: 10.0,
                y: 12.0,
                w: 148.0,
                h: 34.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &button,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("button text entry");
        let text_w = text_width_for_buffer(&entry.buffer);
        let content_left = 10.0 + theme.spacing;
        let content_w = 148.0 - theme.spacing * 2.0;
        let expected_left = content_left + ((content_w - text_w).max(0.0) * 0.5);
        let font_size = text_font_size(&button, &theme, 1.0);
        let line_height = text_line_height(font_size, &theme, 1.0);
        let expected_top = (12.0 + ((34.0 - line_height) * 0.5).max(0.0)).round();

        assert!(
            (entry.left - expected_left).abs() < 0.01,
            "button text should default to horizontal center: left={} expected={expected_left}",
            entry.left
        );
        assert!(
            (entry.top - expected_top).abs() < 0.01,
            "button text should stay vertically centered on a physical pixel: top={} expected={expected_top}",
            entry.top
        );
    }

    #[test]
    fn dropdown_text_centers_field_and_overlay_items_with_custom_font_size() {
        let mut dropdown = node("precision", WidgetKind::Dropdown);
        dropdown.props.items = vec![
            "fp32".to_string(),
            "amp fp16".to_string(),
            "bf16".to_string(),
        ];
        dropdown.props.text = Some("amp fp16".to_string());
        dropdown.style.text.font_size = Some(13.0);

        let rect = Rect {
            x: 20.0,
            y: 30.0,
            w: 160.0,
            h: 33.0,
        };
        let mut layout = LayoutResult::default();
        layout.rects.insert("precision".to_string(), rect);

        let theme = Theme::dark();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();

        let field_state = WidgetState::from_tree(&dropdown);
        let mut field_entries = Vec::new();
        collect_text(
            &dropdown,
            &layout,
            &field_state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut field_entries,
        );

        let font_size = text_font_size(&dropdown, &theme, 1.0);
        let line_height = text_line_height_for_style(&dropdown.style.text, font_size, &theme, 1.0);
        let field = field_entries
            .iter()
            .find(|entry| entry.key.text == "amp fp16")
            .expect("selected dropdown field text");
        let expected_field_top = centered_control_text_top(rect, line_height);
        assert!(
            (field.top - expected_field_top).abs() < 0.01,
            "dropdown field text should be pixel-centered: top={} expected={expected_field_top}",
            field.top
        );

        let mut open_state = field_state;
        open_state.open_dropdown = Some("precision".to_string());
        let mut overlay_entries = Vec::new();
        collect_dropdown_overlay_text(
            &dropdown,
            &layout,
            &open_state,
            &theme,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut overlay_entries,
        );

        let row_h = theme.control_height();
        let bf16_row_y = rect.y + rect.h + 2.0 * row_h;
        let expected_bf16_top = (bf16_row_y + ((row_h - line_height) * 0.5).max(0.0)).round();
        let bf16 = overlay_entries
            .iter()
            .find(|entry| entry.key.text == "bf16")
            .expect("bf16 dropdown overlay text");
        assert!(
            (bf16.top - expected_bf16_top).abs() < 0.01,
            "dropdown overlay text should align with visual row height: top={} expected={expected_bf16_top}",
            bf16.top
        );
    }

    #[test]
    fn number_input_text_color_uses_field_part() {
        let mut number = node("amount", WidgetKind::NumberInput);
        number.style.parts.parts.insert(
            "field".to_string(),
            crate::style::PartStyle {
                text: crate::style::TextStyle {
                    color: Some(crate::style::ColorRef::Rgba([0.25, 0.50, 0.75, 1.0])),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let color = part_text_color(
            &number,
            &WidgetState::default(),
            &Theme::dark(),
            &["field"],
            glyph_color([1.0, 1.0, 1.0, 1.0]),
        );

        assert_eq!(color, Color::rgba(63, 127, 191, 255));
    }

    #[test]
    fn paint_transform_scales_text_entries_around_widget_center() {
        let mut font_system = FontSystem::new();
        let buffer = Buffer::new(&mut font_system, Metrics::new(12.0, 16.0));
        let key = TextKey {
            text: "Run".to_string(),
            font_family: String::new(),
            font_weight: Weight::NORMAL.0,
            font_style: FontStyle::Normal,
            tabular_nums: false,
            letter_spacing_milli: 0,
            font_size_milli: 12000,
            line_height_milli: 16000,
            width_milli: 80000,
            wrap: false,
        };
        let mut entries = vec![TextEntry {
            key,
            buffer,
            left: 20.0,
            top: 10.0,
            scale: 1.0,
            clip: TextBounds {
                left: 18,
                top: 8,
                right: 88,
                bottom: 30,
            },
            untransformed_clip: TextBounds {
                left: 18,
                top: 8,
                right: 88,
                bottom: 30,
            },
            color: Color::rgb(255, 255, 255),
            custom_glyphs: Vec::new(),
        }];

        apply_transform_to_text_entries(
            &mut entries,
            Some(TransformStyle {
                translate_x: 2.0,
                translate_y: 1.0,
                scale_x: 1.5,
                scale_y: 1.5,
                rotate_deg: 45.0,
            }),
            2.0,
            [50.0, 30.0],
        );

        assert_eq!(entries[0].left, 9.0);
        assert_eq!(entries[0].top, 2.0);
        assert_eq!(entries[0].scale, 1.5);
        assert_eq!(entries[0].clip.left, 6);
        assert_eq!(entries[0].clip.top, -1);
        assert_eq!(entries[0].clip.right, 111);
        assert_eq!(entries[0].clip.bottom, 32);
    }

    #[test]
    fn relative_positioned_text_clips_against_painted_offset() {
        let mut font_system = FontSystem::new();
        let buffer = Buffer::new(&mut font_system, Metrics::new(12.0, 16.0));
        let key = TextKey {
            text: "z 2".to_string(),
            font_family: String::new(),
            font_weight: Weight::NORMAL.0,
            font_style: FontStyle::Normal,
            tabular_nums: false,
            letter_spacing_milli: 0,
            font_size_milli: 12000,
            line_height_milli: 16000,
            width_milli: 80000,
            wrap: false,
        };
        let mut entries = vec![TextEntry {
            key,
            buffer,
            left: 20.0,
            top: 10.0,
            scale: 1.0,
            clip: TextBounds {
                left: 10,
                top: 30,
                right: 110,
                bottom: 50,
            },
            untransformed_clip: TextBounds {
                left: 10,
                top: 10,
                right: 110,
                bottom: 50,
            },
            color: Color::rgb(255, 255, 255),
            custom_glyphs: Vec::new(),
        }];

        apply_transform_to_text_entries(
            &mut entries,
            Some(TransformStyle {
                translate_x: 0.0,
                translate_y: 18.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotate_deg: 0.0,
            }),
            1.0,
            [60.0, 30.0],
        );

        assert_eq!(entries[0].top, 28.0);
        assert_eq!(entries[0].clip.top, 30);
        assert_eq!(entries[0].clip.bottom, 68);
    }

    #[test]
    fn paint_clip_applies_after_relative_text_offset() {
        let mut font_system = FontSystem::new();
        let buffer = Buffer::new(&mut font_system, Metrics::new(12.0, 16.0));
        let key = TextKey {
            text: "z 2".to_string(),
            font_family: String::new(),
            font_weight: Weight::NORMAL.0,
            font_style: FontStyle::Normal,
            tabular_nums: false,
            letter_spacing_milli: 0,
            font_size_milli: 12000,
            line_height_milli: 16000,
            width_milli: 80000,
            wrap: false,
        };
        let mut entries = vec![TextEntry {
            key,
            buffer,
            left: 20.0,
            top: 10.0,
            scale: 1.0,
            clip: TextBounds {
                left: 10,
                top: 10,
                right: 110,
                bottom: 50,
            },
            untransformed_clip: TextBounds {
                left: 10,
                top: 10,
                right: 110,
                bottom: 50,
            },
            color: Color::rgb(255, 255, 255),
            custom_glyphs: Vec::new(),
        }];

        apply_transform_to_text_entries(
            &mut entries,
            Some(TransformStyle {
                translate_x: 0.0,
                translate_y: 18.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotate_deg: 0.0,
            }),
            1.0,
            [60.0, 30.0],
        );
        apply_paint_clip_to_text_entries(
            &mut entries,
            Some(Rect {
                x: 0.0,
                y: 30.0,
                w: 200.0,
                h: 40.0,
            }),
        );

        assert_eq!(entries[0].top, 28.0);
        assert_eq!(entries[0].clip.top, 30);
        assert_eq!(entries[0].clip.bottom, 68);
    }

    #[test]
    fn container_transform_propagates_to_child_text_entries() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.transform = Some(TransformStyle {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 2.0,
            scale_y: 2.0,
            rotate_deg: 0.0,
        });
        let mut label = node("label", WidgetKind::Label);
        label.props.text = Some("Child".to_string());
        panel.children.push(label);

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
            "label".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 40.0,
                h: 24.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut transformed = Vec::new();
        collect_text(
            &panel,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut transformed,
        );

        let entry = transformed
            .iter()
            .find(|entry| entry.key.text == "Child")
            .expect("child label text");
        assert_eq!(entry.left, -30.0);
        assert_eq!(entry.scale, 2.0);
    }

    #[test]
    fn label_text_entries_wrap_by_default() {
        let mut label = node("label", WidgetKind::Label);
        label.props.text = Some("This label should wrap inside its narrow layout rect".to_string());

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "label".to_string(),
            Rect {
                x: 8.0,
                y: 10.0,
                w: 120.0,
                h: 72.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &label,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("label text entry");
        assert!(entry.key.wrap, "label text should use the wrapping path");
        assert_eq!(entry.top, 10.0);
    }

    #[test]
    fn clipped_label_visible_rect_suppresses_text_entry() {
        let mut label = node("label", WidgetKind::Label);
        label.props.text = Some("Row 08".to_string());

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "label".to_string(),
            Rect {
                x: 20.0,
                y: 220.0,
                w: 96.0,
                h: 24.0,
            },
        );
        layout.clips.insert(
            "label".to_string(),
            Rect {
                x: 20.0,
                y: 220.0,
                w: 0.0,
                h: 0.0,
            },
        );
        layout.paint_clips.insert(
            "label".to_string(),
            Rect {
                x: 20.0,
                y: 100.0,
                w: 160.0,
                h: 96.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &label,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        assert!(
            entries.is_empty(),
            "fully clipped labels should not submit text entries"
        );
    }

    #[test]
    fn single_line_label_text_centers_vertically_in_tall_rect() {
        let mut label = node("label", WidgetKind::Label);
        label.props.text = Some("80,000 rows".to_string());

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "label".to_string(),
            Rect {
                x: 8.0,
                y: 10.0,
                w: 102.0,
                h: 34.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &label,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("label text entry");
        assert!(
            (entry.top - 17.5).abs() < 0.01,
            "single-line label should center vertically: top={}",
            entry.top
        );
    }

    #[test]
    fn label_text_entries_can_disable_wrap() {
        let mut label = node("label", WidgetKind::Label);
        label.props.text = Some("Single line".to_string());
        label.props.wrap = Some(false);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "label".to_string(),
            Rect {
                x: 8.0,
                y: 10.0,
                w: 120.0,
                h: 32.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &label,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("label text entry");
        assert!(
            !entry.key.wrap,
            "label wrap=false should keep single-line path"
        );
    }

    #[test]
    fn non_wrapping_text_area_applies_horizontal_scroll_offset() {
        let mut area = node("code", WidgetKind::TextArea);
        area.props.wrap = Some(false);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "code".to_string(),
            Rect {
                x: 8.0,
                y: 10.0,
                w: 180.0,
                h: 48.0,
            },
        );

        let mut state = WidgetState::default();
        state.text_val.insert(
            "code".to_string(),
            "alpha.beta.gamma.delta.epsilon".to_string(),
        );
        state.text_cursor.insert("code".to_string(), 22);
        state.text_scroll_x.insert("code".to_string(), 42.0);
        state.text_scroll_y.insert("code".to_string(), 0.0);

        let theme = Theme::dark();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &area,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("text area entry");
        let expected_left = 8.0 + theme.spacing - 42.0;
        assert!(
            (entry.left - expected_left).abs() < 0.01,
            "non-wrapping text should shift left by horizontal scroll: left={} expected={expected_left}",
            entry.left
        );
        let caret_x = caret_positions
            .get("code")
            .map(|xy| xy[0])
            .expect("caret position");
        assert!(
            caret_x < text_width_for_buffer(&entry.buffer),
            "caret position should be recorded after horizontal scroll adjustment"
        );
    }

    #[test]
    fn active_modal_text_collection_splits_base_and_overlay_text() {
        let mut background = node("background", WidgetKind::Label);
        background.props.text = Some("Background copy".to_string());
        let mut modal_label = node("modal-label", WidgetKind::Label);
        modal_label.props.text = Some("Modal copy".to_string());
        let mut modal = node("modal", WidgetKind::Modal);
        modal.props.open = Some(true);
        modal.children = vec![modal_label];
        let mut root = node("window", WidgetKind::Window);
        root.children = vec![background, modal];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 240.0,
            },
        );
        layout.rects.insert(
            "background".to_string(),
            Rect {
                x: 12.0,
                y: 12.0,
                w: 180.0,
                h: 32.0,
            },
        );
        layout.rects.insert(
            "modal".to_string(),
            Rect {
                x: 100.0,
                y: 70.0,
                w: 200.0,
                h: 120.0,
            },
        );
        layout.rects.insert(
            "modal-label".to_string(),
            Rect {
                x: 116.0,
                y: 92.0,
                w: 160.0,
                h: 32.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut base_entries = Vec::new();

        collect_text(
            &root,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            true,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut base_entries,
        );

        assert!(
            base_entries
                .iter()
                .any(|entry| entry.key.text == "Background copy"),
            "background text should remain in the base pass while a modal is active"
        );
        assert!(
            base_entries
                .iter()
                .all(|entry| entry.key.text != "Modal copy"),
            "open modal text should be withheld from the base pass"
        );

        let mut overlay_entries = Vec::new();
        let modal = active_open_modal(&root).expect("active modal");

        collect_text(
            modal,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut overlay_entries,
        );

        assert!(
            overlay_entries
                .iter()
                .any(|entry| entry.key.text == "Modal copy"),
            "modal text should be collected into the overlay pass"
        );
    }

    #[test]
    fn menu_overlay_text_does_not_remove_underlying_base_text() {
        let mut menu = node("file-menu", WidgetKind::Menu);
        menu.props.text = Some("File".to_string());
        let mut background = node("background", WidgetKind::Label);
        background.props.text = Some("Text under menu popup".to_string());
        let mut root = node("window", WidgetKind::Window);
        root.children = vec![menu, background];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 240.0,
            },
        );
        layout.rects.insert(
            "file-menu".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 70.0,
                h: 30.0,
            },
        );
        layout.rects.insert(
            "background".to_string(),
            Rect {
                x: 12.0,
                y: 40.0,
                w: 210.0,
                h: 32.0,
            },
        );

        let theme = Theme::dark();
        let mut state = WidgetState {
            open_menu: Some("file-menu".to_string()),
            ..Default::default()
        };
        state.menu_items.insert(
            "file-menu".to_string(),
            vec![NavigationItem {
                id: "open-item".to_string(),
                value: "Open".to_string(),
                disabled: false,
            }],
        );
        let menu_overlays = active_menu_overlay_rects(&root, &layout, &state, &theme, 1.0);
        assert!(menu_overlays[0].is_some());

        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();

        collect_text(
            &root,
            &layout,
            &state,
            &theme,
            None,
            None,
            menu_overlays,
            None,
            &[],
            true,
            &mut font_system,
            &font_aliases,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        assert!(
            entries
                .iter()
                .any(|entry| entry.key.text == "Text under menu popup"),
            "menu popup should render above existing text without deleting it from the base pass"
        );

        collect_menu_overlay_text(
            &root,
            &layout,
            &state,
            &theme,
            &mut font_system,
            &font_aliases,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        assert!(
            entries.iter().any(|entry| entry.key.text == "Open"),
            "menu item text should still be collected into the overlay pass"
        );
    }

    #[test]
    fn modal_title_text_centers_in_header_band() {
        let mut modal = node("modal", WidgetKind::Modal);
        modal.props.open = Some(true);
        modal.props.text = Some("Modal probe".to_string());
        modal.style.layout.padding = Some(16.0);
        modal.style.visual.border_width = Some(2.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "modal".to_string(),
            Rect {
                x: 50.0,
                y: 40.0,
                w: 220.0,
                h: 140.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &modal,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let title = entries
            .iter()
            .find(|entry| entry.key.text == "Modal probe")
            .expect("modal title text");
        let line_height = text_line_height(text_font_size(&modal, &theme, 1.0), &theme, 1.0);
        let title_band_h = panel_title_padding(&modal, &theme, 1.0).top + line_height;
        let expected_top = 40.0 + 2.0 + ((title_band_h - line_height) * 0.5).max(0.0);

        assert!(
            (title.top - expected_top).abs() < 0.01,
            "modal title should center in the header band: top={} expected={expected_top}",
            title.top
        );
    }

    #[test]
    fn generated_before_text_reserves_space_before_label_text() {
        let mut label = node("label", WidgetKind::Label);
        label.props.text = Some("Declaration query".to_string());
        label.props.wrap = Some(false);
        label.style.parts.parts.insert(
            "before".to_string(),
            PartStyle {
                content: Some(GeneratedContent::Text("PASS ".to_string())),
                text: TextStyle {
                    font_weight: Some(Weight::BOLD.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "label".to_string(),
            Rect {
                x: 8.0,
                y: 10.0,
                w: 220.0,
                h: 32.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &label,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let main = entries
            .iter()
            .find(|entry| entry.key.text == "Declaration query")
            .expect("main label text");
        let before = entries
            .iter()
            .find(|entry| entry.key.text == "PASS ")
            .expect("generated before text");

        assert!(
            main.left > before.left + 24.0,
            "generated before text did not reserve inline space: before_left={} main_left={}",
            before.left,
            main.left
        );
    }

    #[test]
    fn padded_badge_center_text_stays_inside_clip() {
        let mut badge = node("badge", WidgetKind::Badge);
        badge.props.text = Some("margin auto".to_string());
        badge.style.layout.padding_left = Some(10.0);
        badge.style.layout.padding_right = Some(10.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "badge".to_string(),
            Rect {
                x: 8.0,
                y: 10.0,
                w: 124.0,
                h: 28.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &badge,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("badge text entry");
        let text_right = entry.left + text_width_for_buffer(&entry.buffer);
        assert!(
            text_right <= entry.clip.right as f32,
            "badge text should stay inside clip: text_right={text_right}, clip_right={}",
            entry.clip.right
        );
    }

    #[test]
    fn standalone_badge_text_uses_pill_height_line_box() {
        let mut badge = node("badge", WidgetKind::Badge);
        badge.props.text = Some("latency p95".to_string());

        let mut layout = LayoutResult::default();
        let rect = Rect {
            x: 8.0,
            y: 10.0,
            w: 144.0,
            h: 23.0,
        };
        layout.rects.insert("badge".to_string(), rect);

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &badge,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("badge text entry");
        let line_height = entry.key.line_height_milli as f32 / 1000.0;
        assert!(
            (line_height - 20.0).abs() < 0.001,
            "default 12px badge text should use the default 20px pill height as line-height, got {line_height}"
        );
        let expected = standalone_badge_text_top(rect, line_height);
        assert_eq!(expected, 12.0);
        assert!(
            (entry.top - expected).abs() < 0.001,
            "filled badge text should center the font metrics inside the pill height: top={} expected={expected}",
            entry.top
        );
    }

    #[test]
    fn standalone_tag_text_uses_same_pill_height_line_box() {
        let mut tag = node("tag", WidgetKind::Tag);
        tag.props.text = Some("Scatter3D".to_string());

        let mut layout = LayoutResult::default();
        let rect = Rect {
            x: 8.0,
            y: 10.0,
            w: 112.0,
            h: 23.0,
        };
        layout.rects.insert("tag".to_string(), rect);

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &tag,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("tag text entry");
        let line_height = entry.key.line_height_milli as f32 / 1000.0;
        let expected = standalone_badge_text_top(rect, line_height);
        assert_eq!(expected, 12.0);
        assert!(
            (entry.top - expected).abs() < 0.001,
            "tag text should use the same pill-height line box as badges: top={} expected={expected}",
            entry.top
        );
    }

    #[test]
    fn terminal_badge_font_family_does_not_change_vertical_line_box() {
        let mut badge = node("badge", WidgetKind::Badge);
        badge.props.text = Some("error rate".to_string());
        badge.style.text.font_family = Some(FontFamily::Name("Consolas".to_string()));
        badge.style.text.font_size = Some(14.0);

        let mut layout = LayoutResult::default();
        let rect = Rect {
            x: 8.0,
            y: 10.0,
            w: 120.0,
            h: 22.0,
        };
        layout.rects.insert("badge".to_string(), rect);

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &badge,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            theme.spacing,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let entry = entries.first().expect("badge text entry");
        let line_height = entry.key.line_height_milli as f32 / 1000.0;
        assert!(
            (line_height - 22.0).abs() < 0.001,
            "14px terminal badges should use the 22px pill height as line-height, got {line_height}"
        );
        assert!(
            (entry.top - rect.y).abs() < 0.001,
            "terminal font metrics should be centered by the text engine inside the pill line box"
        );
    }

    #[test]
    fn panel_generated_after_anchors_to_title_band() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.layout.padding = Some(12.0);
        panel.style.parts.parts.insert(
            "after".to_string(),
            PartStyle {
                content: Some(GeneratedContent::Text("STAMP".to_string())),
                layout: PartLayoutStyle {
                    width: Some(72.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "panel".to_string(),
            Rect {
                x: 8.0,
                y: 10.0,
                w: 220.0,
                h: 160.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState::default();
        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &panel,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            0.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let stamp = entries
            .iter()
            .find(|entry| entry.key.text == "STAMP")
            .expect("generated panel after text");

        assert!(
            stamp.top < 40.0,
            "panel generated text should anchor near the title band, got top={}",
            stamp.top
        );
        assert!(
            stamp.clip.bottom < 50,
            "panel generated text clip should stay in the title band, got bottom={}",
            stamp.clip.bottom
        );
    }

    #[test]
    fn table_tooltip_overlay_does_not_remove_underlying_text() {
        let table = node("table", WidgetKind::DataFrameTable);
        let theme = Theme::dark();
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "table".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 420.0,
                h: 160.0,
            },
        );

        let mut state = WidgetState::default();
        state.tables.insert(
            "table".to_string(),
            TableState {
                columns: vec!["a".to_string(), "b".to_string()],
                dtypes: vec!["f32".to_string(), "f32".to_string()],
                rows: 3,
                resource_id: None,
                page_size: 10,
                scroll_row: 0,
                scroll_col: 0,
                selected: None,
                sort: None,
                row_order: None,
                column_widths: Vec::new(),
            },
        );

        let mut font_system = FontSystem::new();
        let resources = ResourceRegistry::default();
        let mut cache = TextBufferCache::default();
        let font_aliases = FontFamilyAliases::default();
        let mut caret_positions = HashMap::new();
        let mut unobscured = Vec::new();
        collect_table_text(
            &table,
            &layout,
            &state,
            &resources,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut unobscured,
        );

        let metrics = table::metrics_for_node(&table, &theme, 1.0);
        let mut with_tooltip_overlay = Vec::new();
        collect_table_text(
            &table,
            &layout,
            &state,
            &resources,
            &theme,
            None,
            None,
            [None, None],
            Some(Rect {
                x: 0.0,
                y: metrics.header_h,
                w: metrics.index_w * 0.5,
                h: metrics.row_h,
            }),
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut with_tooltip_overlay,
        );

        assert_eq!(unobscured.len(), with_tooltip_overlay.len());
        assert!(!with_tooltip_overlay.is_empty());
    }

    #[test]
    fn tooltip_overlay_text_does_not_remove_underlying_base_text() {
        let mut target = node("target", WidgetKind::Button);
        target.props.text = Some("Target".to_string());
        target.props.tooltip = Some("Tooltip copy".to_string());
        let mut background = node("background", WidgetKind::Label);
        background.props.text = Some("Text under tooltip".to_string());
        let mut root = node("window", WidgetKind::Window);
        root.children = vec![target, background];

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 240.0,
            },
        );
        layout.rects.insert(
            "target".to_string(),
            Rect {
                x: 12.0,
                y: 12.0,
                w: 90.0,
                h: 30.0,
            },
        );
        layout.rects.insert(
            "background".to_string(),
            Rect {
                x: 20.0,
                y: 50.0,
                w: 220.0,
                h: 30.0,
            },
        );

        let theme = Theme::dark();
        let state = WidgetState {
            hovered: Some("target".to_string()),
            ..Default::default()
        };
        let tooltip_overlay = active_tooltip_overlay_rect(&root, &layout, &theme, &state, 1.0);
        assert!(tooltip_overlay.is_some());

        let mut font_system = FontSystem::new();
        let font_aliases = FontFamilyAliases::default();
        let mut cache = TextBufferCache::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_text(
            &root,
            &layout,
            &state,
            &theme,
            None,
            None,
            [None, None],
            tooltip_overlay,
            &[],
            true,
            &mut font_system,
            &font_aliases,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        assert!(
            entries
                .iter()
                .any(|entry| entry.key.text == "Text under tooltip"),
            "tooltip overlays should paint above existing text without deleting it from the base pass"
        );
    }

    #[test]
    fn table_text_anchors_to_full_rect_when_parent_clip_scrolls() {
        let table = node("table", WidgetKind::DataFrameTable);
        let theme = Theme::dark();
        let full_rect = Rect {
            x: 0.0,
            y: 20.0,
            w: 420.0,
            h: 160.0,
        };
        let visible_clip = Rect {
            x: 0.0,
            y: 42.0,
            w: 420.0,
            h: 138.0,
        };
        let mut layout = LayoutResult::default();
        layout.rects.insert("table".to_string(), full_rect);
        layout.clips.insert("table".to_string(), visible_clip);
        layout.paint_clips.insert("table".to_string(), visible_clip);

        let mut state = WidgetState::default();
        state.tables.insert(
            "table".to_string(),
            TableState {
                columns: vec!["a".to_string(), "b".to_string()],
                dtypes: vec!["f32".to_string(), "f32".to_string()],
                rows: 3,
                resource_id: None,
                page_size: 10,
                scroll_row: 0,
                scroll_col: 0,
                selected: None,
                sort: None,
                row_order: None,
                column_widths: Vec::new(),
            },
        );

        let mut font_system = FontSystem::new();
        let resources = ResourceRegistry::default();
        let mut cache = TextBufferCache::default();
        let font_aliases = FontFamilyAliases::default();
        let mut caret_positions = HashMap::new();
        let mut entries = Vec::new();
        collect_table_text(
            &table,
            &layout,
            &state,
            &resources,
            &theme,
            None,
            None,
            [None, None],
            None,
            &[],
            false,
            &mut font_system,
            &font_aliases,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );

        let metrics = table::metrics_for_node(&table, &theme, 1.0);
        let row_font_size = text_font_size(&table, &theme, 1.0);
        let row_line_height =
            text_line_height_for_parts(&table, &["row"], row_font_size, &theme, 1.0);
        let expected_top =
            full_rect.y + metrics.header_h + ((metrics.row_h - row_line_height) * 0.5).max(0.0);
        let clipped_top =
            visible_clip.y + metrics.header_h + ((metrics.row_h - row_line_height) * 0.5).max(0.0);
        let row_index = entries
            .iter()
            .find(|entry| entry.key.text == "0")
            .expect("first row index text");

        assert!(
            (row_index.top - expected_top).abs() < 0.01,
            "row text should stay anchored to full table rect after parent clipping: top={} expected={expected_top}",
            row_index.top
        );
        assert!(
            (row_index.top - clipped_top).abs() > 1.0,
            "test should distinguish full rect from visible clip anchoring"
        );
    }

    #[test]
    fn local_font_family_alias_matches_loaded_system_family() {
        let font_system = FontSystem::new();
        let Some(first_family) = font_system
            .db()
            .faces()
            .find_map(|face| face.families.first().map(|(name, _)| name.clone()))
        else {
            return;
        };

        assert_eq!(
            local_font_family_alias(&font_system, &first_family).as_deref(),
            Some(first_family.as_str())
        );
        assert_eq!(
            local_font_family_alias(&font_system, &first_family.to_ascii_lowercase()).as_deref(),
            Some(first_family.as_str())
        );
    }

    #[test]
    fn font_source_resolves_base64_data_font_url() {
        let source = resolve_font_source("data:font/ttf;base64,AAEAAA==", Some("truetype"));

        match source {
            Ok(ResolvedFontSource::Data(data)) => assert_eq!(data, b"\0\x01\0\0"),
            _ => panic!("expected decoded sfnt font data"),
        }
    }

    #[test]
    fn font_source_resolves_base64_woff_font_url() {
        let woff = minimal_woff_font_data();
        let url = format!(
            "data:font/woff;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(woff)
        );
        let source = resolve_font_source(&url, Some("woff"));

        match source {
            Ok(ResolvedFontSource::Data(data)) => assert!(is_supported_sfnt_font_data(&data)),
            _ => panic!("expected decoded WOFF font data"),
        }
    }

    #[test]
    fn font_source_rejects_unsupported_data_font_payload() {
        let source = resolve_font_source("data:font/woff2;base64,d09GMg==", None);

        assert!(matches!(
            source,
            Err(
                "unsupported font data; only sfnt TrueType/OpenType/TTC or WOFF1 data is supported"
            )
        ));
    }

    #[test]
    fn font_source_rejects_remote_urls() {
        let source = resolve_font_source("https://example.com/font.ttf", None);

        assert!(matches!(source, Err("remote font URLs are not supported")));
    }

    #[test]
    fn font_source_rejects_unsupported_declared_format() {
        let source = resolve_font_source("data:font/woff2;base64,d09GMg==", Some("woff2"));

        assert!(matches!(
            source,
            Err(
                "unsupported font format; only truetype, opentype, collection, and woff are supported"
            )
        ));
    }

    #[test]
    fn font_source_file_url_decodes_percent_escaped_path() {
        let path = font_source_path("file:///C:/Demo%20Fonts/Report%20UI.ttf?cache=1");

        assert!(path.to_string_lossy().contains("Demo Fonts"));
        assert!(path.to_string_lossy().contains("Report UI.ttf"));
    }

    fn minimal_woff_font_data() -> Vec<u8> {
        let mut data = Vec::new();
        write_be_u32(&mut data, u32::from_be_bytes(*b"wOFF"));
        write_be_u32(&mut data, u32::from_be_bytes(*b"OTTO"));
        write_be_u32(&mut data, 68);
        write_be_u16(&mut data, 1);
        write_be_u16(&mut data, 0);
        write_be_u32(&mut data, 32);
        write_be_u16(&mut data, 0);
        write_be_u16(&mut data, 0);
        write_be_u32(&mut data, 0);
        write_be_u32(&mut data, 0);
        write_be_u32(&mut data, 0);
        write_be_u32(&mut data, 0);
        write_be_u32(&mut data, 0);
        data.extend_from_slice(b"head");
        write_be_u32(&mut data, 64);
        write_be_u32(&mut data, 4);
        write_be_u32(&mut data, 4);
        write_be_u32(&mut data, 0);
        data.extend_from_slice(b"OTTO");
        data
    }
}
