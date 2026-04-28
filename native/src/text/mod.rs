use std::borrow::Cow;
use std::collections::HashMap;

use glyphon::cosmic_text::{FeatureTag, FontFeatures};
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping,
    Style as GlyphStyle, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
    Weight, Wrap,
};

use crate::css_style::{computed_style_for_virtual_element, StylesheetStore};
use crate::document::{WidgetKind, WidgetNode};
use crate::events::WidgetState;
use crate::layout::{LayoutResult, Rect};
use crate::overlays::{
    active_menu_overlay_rects, active_tooltip_overlay_rect, dropdown_overlay_rect, find_node,
    menu_popup_rect, rich_tooltip_target, tooltip_target,
};
use crate::resources::ResourceRegistry;
use crate::style::{
    badge_font_size_lp, badge_height_for_style, badge_width_for_text, base_part_style,
    checked_part_style_for_state, collapsed_part_style_for_state,
    collapsible_header_height_for_style, expanded_part_style_for_state,
    number_stepper_width_for_style, open_part_style_for_state, selected_part_style_for_state,
    state_part_style_for_state, uniform_layout_padding, FontFamily, FontStyle, FontVariantNumeric,
    LineHeight, NodeStyle, TextAlign, TextOverflow, TextSpacing, TextStyle, TextTransform,
    VisualStyle, BADGE_GAP_LP, BORDER_WIDTH_LP, CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP,
    DROPDOWN_CHEVRON_WIDTH_LP, PANEL_ACCENT_WIDTH_LP, TAB_GAP_LP,
};
use crate::table;
use crate::theme::Theme;
use crate::toast::{toast_colors, toast_padding, toast_rect, toast_stack_index, ToastOverlay};

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
    clip: TextBounds,
    color: Color,
}

type TextBufferCache = HashMap<TextKey, Vec<Buffer>>;

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
    swash_cache: SwashCache,
    atlas: TextAtlas,
    renderer: TextRenderer,
    viewport: Viewport,
    entries: Vec<TextEntry>,
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
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        );
        Self {
            font_system,
            swash_cache,
            atlas,
            renderer,
            viewport,
            entries: Vec::new(),
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
        window_w: f32,
        window_h: f32,
    ) -> HashMap<String, [f32; 2]> {
        let pad = theme.spacing * sf;
        let open_dropdown = state.open_dropdown.as_deref();
        let dropdown_overlay = dropdown_overlay_rect(layout, state, theme, sf);
        let menu_overlays = active_menu_overlay_rects(tree, layout, state, theme, sf);
        let tooltip_overlay = active_tooltip_overlay_rect(tree, layout, theme, state, sf);

        let mut entries = std::mem::take(&mut self.entries);
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
            &mut self.font_system,
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
            &mut self.font_system,
            sf,
            pad,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );
        collect_dropdown_overlay_text(
            tree,
            layout,
            state,
            theme,
            &mut self.font_system,
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
            sf,
            stylesheets,
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
            sf,
            pad,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );
        collect_toast_text(
            toasts,
            theme,
            &mut self.font_system,
            sf,
            window_w,
            window_h,
            stylesheets,
            &mut cache,
            &mut caret_positions,
            &mut entries,
        );
        self.entries = entries;
        caret_positions
    }

    /// Upload glyph data to the GPU.  Call this once per frame before `render`.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.entries.is_empty() {
            return;
        }

        // Destructure to obtain separate mutable/shared borrows of each field.
        let TextRendererDg {
            renderer,
            font_system,
            atlas,
            viewport,
            swash_cache,
            entries,
            ..
        } = self;

        let areas: Vec<TextArea<'_>> = entries
            .iter()
            .map(|e| TextArea {
                buffer: &e.buffer,
                left: e.left,
                top: e.top,
                scale: 1.0,
                bounds: e.clip,
                default_color: e.color,
                custom_glyphs: &[],
            })
            .collect();

        if let Err(e) = renderer.prepare(
            device,
            queue,
            font_system,
            atlas,
            viewport,
            areas,
            swash_cache,
        ) {
            eprintln!("glyphon prepare error: {e}");
        }
    }

    /// Record text draw calls into an active render pass.
    ///
    /// Takes `&self` because `TextRenderer::render` only reads state.
    pub fn render<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>) {
        if self.entries.is_empty() {
            return;
        }
        if let Err(e) = self.renderer.render(&self.atlas, &self.viewport, pass) {
            eprintln!("glyphon render error: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Widget-tree → TextEntry mapping
// ---------------------------------------------------------------------------

fn collect_text(
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
    font_system: &mut FontSystem,
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
    let primary_part_text = match node.kind {
        WidgetKind::ProgressBar => base_part_style(&node.style, "label").map(|part| &part.text),
        WidgetKind::Tab => base_part_style(&node.style, "tab").map(|part| &part.text),
        WidgetKind::NavItem => base_part_style(&node.style, "item").map(|part| &part.text),
        WidgetKind::Collapsible => base_part_style(&node.style, "header").map(|part| &part.text),
        _ => None,
    };
    let font_size = primary_part_text
        .and_then(|text| text.font_size)
        .map(|font_size| font_size.max(8.0) * sf)
        .unwrap_or_else(|| text_font_size(node, theme, sf));
    let line_height =
        text_line_height_from_styles(primary_part_text, &node.style.text, font_size, theme, sf);
    let font_family = primary_part_text
        .and_then(|text| text.font_family.as_ref())
        .or(node.style.text.font_family.as_ref());
    let font_weight = primary_part_text
        .and_then(|text| text.font_weight)
        .or(node.style.text.font_weight)
        .unwrap_or(Weight::NORMAL.0);
    let align = primary_part_text
        .and_then(|text| text.text_align)
        .or(node.style.text.text_align)
        .unwrap_or(TextAlign::Left);
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
            | WidgetKind::Checkbox
            | WidgetKind::Collapsible
            | WidgetKind::Dropdown
            | WidgetKind::Menu
            | WidgetKind::TextInput
            | WidgetKind::TextArea
            | WidgetKind::NumberInput
            | WidgetKind::ProgressBar
            | WidgetKind::Tab
            | WidgetKind::NavItem
    );

    if is_text_widget {
        let mut caret = None;
        if matches!(
            node.kind,
            WidgetKind::TextInput | WidgetKind::TextArea | WidgetKind::NumberInput
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
                            let min_top =
                                r.y + (BORDER_WIDTH_LP + PANEL_ACCENT_WIDTH_LP + 2.0) * sf;
                            text_top = text_top.max(min_top);
                        }
                        (
                            r.x + title_pad.left,
                            text_top,
                            r.x + title_pad.left,
                            r.y,
                            r.x + r.w - title_pad.right,
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
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            r.x + pad,
                            top,
                            r.x + pad,
                            r.y,
                            chevron_left - pad * 0.5,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::Button => {
                        let reserved = badge_reserved_width(node, theme, sf);
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            r.x + pad,
                            top,
                            r.x + pad,
                            r.y,
                            r.x + r.w - pad - reserved,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::Badge | WidgetKind::Tag => {
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (r.x + pad, top, r.x + pad, r.y, r.x + r.w - pad, r.y + r.h)
                    }
                    WidgetKind::NumberInput => {
                        let step_w = number_stepper_width_for_style(&node.style, r.w, sf);
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            r.x + pad,
                            top,
                            r.x + pad,
                            r.y,
                            r.x + r.w - step_w - pad * 0.5,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::TextArea => {
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
                    _ => {
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
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
                } else if node.kind == WidgetKind::Checkbox && !placeholder {
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
                    WidgetKind::ProgressBar | WidgetKind::Badge | WidgetKind::Tag
                ) && node.style.text.text_align.is_none()
                {
                    TextAlign::Center
                } else {
                    align
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
                    ) {
                        let text_bounds = TextBounds {
                            left: clip_rect.x as i32,
                            top: clip_rect.y as i32,
                            right: (clip_rect.x + clip_rect.w) as i32,
                            bottom: (clip_rect.y + clip_rect.h) as i32,
                        };
                        if node.kind == WidgetKind::TextArea {
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
                            push_text_entry_impl(
                                font_system,
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
                                scroll_y,
                                cache,
                                if placeholder { None } else { caret },
                                caret_positions,
                                text_options,
                            );
                        } else {
                            push_text_entry(
                                font_system,
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
            WidgetKind::Button | WidgetKind::Tab | WidgetKind::NavItem
        ) {
            emit_badge_text(
                node,
                layout,
                theme,
                sf,
                state,
                font_system,
                out,
                cache,
                caret_positions,
                open_dropdown,
                dropdown_overlay,
                menu_overlays,
                tooltip_overlay,
            );
        }

        if node.kind == WidgetKind::NumberInput {
            if let Some(r) = layout.rects.get(&node.id) {
                let Some(node_clip) = layout.visible_rect(&node.id) else {
                    return;
                };
                let step_w = number_stepper_width_for_style(&node.style, r.w, sf);
                let step_rect = Rect {
                    x: r.x + r.w - step_w,
                    y: r.y,
                    w: step_w,
                    h: r.h,
                };
                let Some(step_clip) = step_rect.intersect(node_clip) else {
                    return;
                };
                if r.w > 0.0
                    && r.h > 0.0
                    && !is_obscured_by_overlay(
                        node,
                        &step_clip,
                        open_dropdown,
                        dropdown_overlay,
                        menu_overlays,
                        tooltip_overlay,
                    )
                {
                    let step_x = r.x + r.w - step_w;
                    let step_left = step_x + BORDER_WIDTH_LP * sf;
                    let half_h = r.h * 0.5;
                    let up_color = number_stepper_text_color(node, state, theme, "stepper-up");
                    let down_color = number_stepper_text_color(node, state, theme, "stepper-down");
                    push_text_entry(
                        font_system,
                        out,
                        "+",
                        font_size,
                        line_height,
                        font_family,
                        font_weight,
                        step_left,
                        r.y + ((half_h - line_height) * 0.5).max(0.0),
                        TextBounds {
                            left: step_clip.x as i32,
                            top: step_clip.y as i32,
                            right: (step_clip.x + step_clip.w) as i32,
                            bottom: (r.y + half_h).min(step_clip.y + step_clip.h) as i32,
                        },
                        up_color,
                        TextAlign::Center,
                        cache,
                        None,
                        caret_positions,
                        text_options,
                    );
                    push_text_entry(
                        font_system,
                        out,
                        "-",
                        font_size,
                        line_height,
                        font_family,
                        font_weight,
                        step_left,
                        r.y + half_h + ((half_h - line_height) * 0.5).max(0.0),
                        TextBounds {
                            left: step_clip.x as i32,
                            top: (r.y + half_h).max(step_clip.y) as i32,
                            right: (step_clip.x + step_clip.w) as i32,
                            bottom: (step_clip.y + step_clip.h) as i32,
                        },
                        down_color,
                        TextAlign::Center,
                        cache,
                        None,
                        caret_positions,
                        text_options,
                    );
                }
            }
        }

        if node.kind == WidgetKind::Dropdown {
            if let Some(r) = layout.rects.get(&node.id) {
                let Some(node_clip) = layout.visible_rect(&node.id) else {
                    return;
                };
                let chevron_w = dropdown_chevron_width(node, font_size, theme, sf);
                let chevron_left = r.x + r.w - pad - chevron_w;
                let chevron_rect = Rect {
                    x: chevron_left,
                    y: r.y,
                    w: chevron_w,
                    h: r.h,
                };
                let Some(chevron_clip) = chevron_rect.intersect(node_clip) else {
                    return;
                };
                if r.w > 0.0
                    && r.h > 0.0
                    && !is_obscured_by_overlay(
                        node,
                        &chevron_clip,
                        open_dropdown,
                        dropdown_overlay,
                        menu_overlays,
                        tooltip_overlay,
                    )
                {
                    let color = part_text_color(
                        node,
                        state,
                        theme,
                        &["chevron"],
                        glyph_color(theme.muted_text),
                    );
                    push_text_entry(
                        font_system,
                        out,
                        "v",
                        font_size,
                        line_height,
                        font_family,
                        font_weight,
                        chevron_left,
                        r.y + ((r.h - line_height) * 0.5).max(0.0),
                        TextBounds {
                            left: chevron_clip.x as i32,
                            top: chevron_clip.y as i32,
                            right: (chevron_clip.x + chevron_clip.w) as i32,
                            bottom: (chevron_clip.y + chevron_clip.h) as i32,
                        },
                        color,
                        TextAlign::Center,
                        cache,
                        None,
                        caret_positions,
                        text_options,
                    );
                }
            }
        }

        if node.kind == WidgetKind::Collapsible {
            if let Some(r) = layout.rects.get(&node.id) {
                let Some(node_clip) = layout.visible_rect(&node.id) else {
                    return;
                };
                let header_h = collapsible_header_height_for_style(&node.style, theme, sf).min(r.h);
                let indicator_w = node
                    .style
                    .parts
                    .parts
                    .get("indicator")
                    .and_then(|part| part.layout.width)
                    .unwrap_or(16.0)
                    .max(1.0)
                    * sf;
                let indicator_rect = Rect {
                    x: r.x + pad,
                    y: r.y,
                    w: indicator_w,
                    h: header_h,
                };
                let Some(indicator_clip) = indicator_rect.intersect(node_clip) else {
                    return;
                };
                if r.w > 0.0 && header_h > 0.0 {
                    let color = part_text_color(
                        node,
                        state,
                        theme,
                        &["indicator"],
                        glyph_color(theme.muted_text),
                    );
                    push_text_entry(
                        font_system,
                        out,
                        if state.is_expanded(&node.id) {
                            "v"
                        } else {
                            ">"
                        },
                        font_size,
                        line_height,
                        font_family,
                        font_weight,
                        indicator_rect.x,
                        r.y + ((header_h - line_height) * 0.5).max(0.0),
                        TextBounds {
                            left: indicator_clip.x as i32,
                            top: indicator_clip.y as i32,
                            right: (indicator_clip.x + indicator_clip.w) as i32,
                            bottom: (indicator_clip.y + indicator_clip.h) as i32,
                        },
                        color,
                        TextAlign::Center,
                        cache,
                        None,
                        caret_positions,
                        text_options,
                    );
                }
            }
        }
    }

    for child in &node.children {
        collect_text(
            child,
            layout,
            state,
            theme,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            tooltip_overlay,
            font_system,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    }
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

fn is_obscured_by_overlay(
    node: &WidgetNode,
    r: &Rect,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
) -> bool {
    if open_dropdown == Some(node.id.as_str()) {
        return false;
    }
    dropdown_overlay.is_some_and(|overlay| rects_intersect(*r, overlay))
        || menu_overlays
            .iter()
            .flatten()
            .any(|overlay| rects_intersect(*r, *overlay))
        || tooltip_overlay.is_some_and(|overlay| rects_intersect(*r, overlay))
}

fn text_bounds_obscured_by_overlay(
    node: &WidgetNode,
    bounds: TextBounds,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
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
        )
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
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
    changed.then_some(visual)
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
    out: &mut Vec<TextEntry>,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    open_dropdown: Option<&str>,
    dropdown_overlay: Option<Rect>,
    menu_overlays: [Option<Rect>; 2],
    tooltip_overlay: Option<Rect>,
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
    ) {
        return;
    }

    let badge_font_size = badge_font_size_lp(&node.style, theme) * sf;
    let badge_parts = &["badge"];
    let badge_line_height =
        text_line_height_for_parts(node, badge_parts, badge_font_size, theme, sf);
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

fn number_stepper_text_color(
    node: &WidgetNode,
    state: &WidgetState,
    theme: &Theme,
    part: &str,
) -> Color {
    part_text_color(
        node,
        state,
        theme,
        &[part, "stepper"],
        glyph_color(theme.muted_text),
    )
}

fn collect_dropdown_overlay_text(
    node: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    font_system: &mut FontSystem,
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
            let scale = text_scale(font_size, theme);
            let row_h = theme.control_height() * scale;
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
                    out,
                    item,
                    font_size,
                    line_height,
                    font_family,
                    font_weight,
                    r.x + item_pad,
                    y + ((row_h - line_height) * 0.5).max(0.0),
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

    for child in &node.children {
        collect_dropdown_overlay_text(
            child,
            layout,
            state,
            theme,
            font_system,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    }
}

fn collect_menu_overlay_text(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    font_system: &mut FontSystem,
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
    let font_size = text_font_size(node, theme, sf);
    let line_height = text_line_height_for_style(&node.style.text, font_size, theme, sf);
    let font_family = node.style.text.font_family.as_ref();
    let font_weight = node.style.text.font_weight.unwrap_or(Weight::NORMAL.0);
    let text_options = text_options_from_style(&node.style.text);
    let row_h = theme.control_height() * sf;
    for (idx, item) in items.iter().enumerate() {
        let y = rect.y + idx as f32 * row_h;
        let disabled = item.disabled || state.is_disabled(&item.id);
        let color = if disabled {
            glyph_color(theme.muted_text)
        } else {
            glyph_color(theme.text)
        };
        push_text_entry(
            font_system,
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
    sf: f32,
    stylesheets: &StylesheetStore,
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
    let style = computed_style_for_virtual_element(
        WidgetKind::Tooltip,
        "__dg_static_tooltip",
        &["static"],
        stylesheets,
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
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    let Some((node, _rect)) = rich_tooltip_target(tree, layout, state) else {
        return;
    };
    for child in &node.children {
        collect_text(
            child,
            layout,
            state,
            theme,
            open_dropdown,
            dropdown_overlay,
            menu_overlays,
            None,
            font_system,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    }
}

fn collect_toast_text(
    toasts: &[ToastOverlay],
    theme: &Theme,
    font_system: &mut FontSystem,
    sf: f32,
    window_w: f32,
    window_h: f32,
    stylesheets: &StylesheetStore,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    let mut stack_counts = [0usize; 4];
    for toast in toasts {
        let classes = [toast.level.as_str()];
        let style = computed_style_for_virtual_element(
            WidgetKind::Toast,
            toast.id.as_str(),
            &classes,
            stylesheets,
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
    font_system: &mut FontSystem,
    sf: f32,
    pad: f32,
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, [f32; 2]>,
    out: &mut Vec<TextEntry>,
) {
    if node.kind == WidgetKind::DataFrameTable {
        if let (Some(r), Some(table_state)) = (layout.visible_rect(&node.id), state.table(&node.id))
        {
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
                let index_header_bounds = table_text_bounds(
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
                let index_header_bounds =
                    clamp_text_bounds_bottom(index_header_bounds, header_bottom);

                if !text_bounds_obscured_by_overlay(
                    node,
                    index_header_bounds,
                    open_dropdown,
                    dropdown_overlay,
                    menu_overlays,
                    tooltip_overlay,
                ) {
                    push_text_entry(
                        font_system,
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
                    let Some((col_x, col_right)) = table::column_bounds(&r, metrics, col_offset)
                    else {
                        continue;
                    };
                    let name = table_state
                        .columns
                        .get(col)
                        .map(String::as_str)
                        .unwrap_or("");
                    let sort_suffix = table::sort_suffix(table_state, col);
                    let label = if sort_suffix.is_empty() {
                        std::borrow::Cow::Borrowed(name)
                    } else {
                        std::borrow::Cow::Owned(format!("{name}{sort_suffix}"))
                    };
                    let bounds = table_text_bounds(
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
                    let bounds = clamp_text_bounds_bottom(bounds, header_bottom);
                    if !text_bounds_obscured_by_overlay(
                        node,
                        bounds,
                        open_dropdown,
                        dropdown_overlay,
                        menu_overlays,
                        tooltip_overlay,
                    ) {
                        push_text_entry(
                            font_system,
                            out,
                            label.as_ref(),
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
                    ) {
                        push_text_entry(
                            font_system,
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
                            table::column_bounds(&r, metrics, col_offset)
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
                        ) {
                            push_text_entry(
                                font_system,
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

    for child in &node.children {
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
            font_system,
            sf,
            pad,
            cache,
            caret_positions,
            out,
        );
    }
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
        WidgetKind::TextInput | WidgetKind::TextArea | WidgetKind::NumberInput => {
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
        _ => node
            .props
            .text
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(|t| (t, false)),
    }
}

fn push_text_entry(
    font_system: &mut FontSystem,
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
        cache,
        caret,
        caret_positions,
        options,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_wrapped_text_entry(
    font_system: &mut FontSystem,
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
        cache,
        None,
        caret_positions,
        options,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_text_entry_impl(
    font_system: &mut FontSystem,
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
        font_family: font_family_key(font_family),
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
            let attrs = text_attrs(font_family, font_weight, options, font_size);
            buf.set_text(font_system, text, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(font_system, false);
            buf
        });
    if let Some((id, cursor)) = caret {
        let mut xy = caret_xy_for_buffer(&buf, text, cursor);
        xy[1] -= caret_scroll_y;
        caret_positions.insert(id.to_string(), xy);
    }
    let aligned_left = match align {
        TextAlign::Left => left,
        TextAlign::Center => {
            let text_w = text_width_for_buffer(&buf);
            let bounds_w = (clip.right - clip.left).max(1) as f32;
            left + ((bounds_w - text_w).max(0.0) * 0.5)
        }
        TextAlign::Right => {
            let text_w = text_width_for_buffer(&buf);
            let bounds_w = (clip.right - clip.left).max(1) as f32;
            left + (bounds_w - text_w).max(0.0)
        }
    };

    out.push(TextEntry {
        key,
        buffer: buf,
        left: aligned_left,
        top,
        clip,
        color,
    });
}

#[allow(clippy::too_many_arguments)]
fn prepare_display_text<'a>(
    font_system: &mut FontSystem,
    text: &'a str,
    font_size: f32,
    line_height: f32,
    font_family: Option<&FontFamily>,
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
    font_weight: u16,
    options: TextRenderOptions,
) -> f32 {
    let mut buf = Buffer::new(font_system, Metrics::new(font_size, line_height));
    buf.set_size(font_system, Some(1_000_000.0), None);
    buf.set_wrap(font_system, Wrap::None);
    let attrs = text_attrs(font_family, font_weight, options, font_size);
    buf.set_text(font_system, text, &attrs, Shaping::Advanced, None);
    buf.shape_until_scroll(font_system, false);
    text_width_for_buffer(&buf)
}

fn text_attrs(
    font_family: Option<&FontFamily>,
    font_weight: u16,
    options: TextRenderOptions,
    font_size: f32,
) -> Attrs<'_> {
    let glyph_style = match options.font_style.unwrap_or(FontStyle::Normal) {
        FontStyle::Normal => GlyphStyle::Normal,
        FontStyle::Italic => GlyphStyle::Italic,
    };
    let mut attrs = Attrs::new()
        .family(to_glyphon_family(font_family))
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

fn font_family_key(family: Option<&FontFamily>) -> String {
    match family {
        Some(FontFamily::Serif) => "serif".to_string(),
        Some(FontFamily::SansSerif) | None => "sans-serif".to_string(),
        Some(FontFamily::Monospace) => "monospace".to_string(),
        Some(FontFamily::Cursive) => "cursive".to_string(),
        Some(FontFamily::Fantasy) => "fantasy".to_string(),
        Some(FontFamily::Name(name)) => format!("name:{name}"),
    }
}

fn to_glyphon_family(family: Option<&FontFamily>) -> Family<'_> {
    match family {
        Some(FontFamily::Serif) => Family::Serif,
        Some(FontFamily::SansSerif) | None => Family::SansSerif,
        Some(FontFamily::Monospace) => Family::Monospace,
        Some(FontFamily::Cursive) => Family::Cursive,
        Some(FontFamily::Fantasy) => Family::Fantasy,
        Some(FontFamily::Name(name)) => Family::Name(name.as_str()),
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
    use crate::events::TableState;
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
    fn table_tooltip_obscures_only_intersecting_text_bounds() {
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
            },
        );

        let mut font_system = FontSystem::new();
        let resources = ResourceRegistry::default();
        let mut cache = TextBufferCache::default();
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
            &mut font_system,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut unobscured,
        );

        let metrics = table::metrics_for_node(&table, &theme, 1.0);
        let mut partially_obscured = Vec::new();
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
            &mut font_system,
            1.0,
            6.0,
            &mut cache,
            &mut caret_positions,
            &mut partially_obscured,
        );

        assert!(unobscured.len() > partially_obscured.len());
        assert!(!partially_obscured.is_empty());
    }
}
