use std::collections::HashMap;

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight, Wrap,
};

use crate::document::{WidgetKind, WidgetNode};
use crate::events::WidgetState;
use crate::layout::{LayoutResult, Rect};
use crate::overlays::{
    active_menu_overlay_rects, dropdown_overlay_rect, find_node, menu_popup_rect, tooltip_target,
};
use crate::resources::ResourceRegistry;
use crate::style::{
    number_stepper_width_for_style, FontFamily, TextAlign, VisualStyle, BORDER_WIDTH_LP,
    CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP, DROPDOWN_CHEVRON_WIDTH_LP, PANEL_ACCENT_WIDTH_LP,
    TAB_GAP_LP,
};
use crate::table;
use crate::theme::Theme;

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
    ) -> HashMap<String, f32> {
        let pad = theme.spacing * sf;
        let open_dropdown = state.open_dropdown.as_deref();
        let dropdown_overlay = dropdown_overlay_rect(layout, state, theme, sf);
        let menu_overlays = active_menu_overlay_rects(tree, layout, state, theme, sf);
        let tooltip_overlay = tooltip_target(tree, layout, theme, state, sf).map(|(_, rect)| rect);

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
    caret_positions: &mut HashMap<String, f32>,
    out: &mut Vec<TextEntry>,
) {
    if node.kind == WidgetKind::Modal && !node.props.open.unwrap_or(false) {
        return;
    }
    let primary_part_text = match node.kind {
        WidgetKind::ProgressBar => node.style.parts.parts.get("label").map(|part| &part.text),
        WidgetKind::Tab => node.style.parts.parts.get("tab").map(|part| &part.text),
        WidgetKind::NavItem => node.style.parts.parts.get("item").map(|part| &part.text),
        _ => None,
    };
    let font_size = primary_part_text
        .and_then(|text| text.font_size)
        .map(|font_size| font_size.max(8.0) * sf)
        .unwrap_or_else(|| text_font_size(node, theme, sf));
    let line_height = text_line_height(font_size, theme, sf);
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
    let is_text_widget = matches!(
        node.kind,
        WidgetKind::Panel
            | WidgetKind::Modal
            | WidgetKind::Sidebar
            | WidgetKind::Label
            | WidgetKind::Button
            | WidgetKind::Checkbox
            | WidgetKind::Dropdown
            | WidgetKind::Menu
            | WidgetKind::TextInput
            | WidgetKind::NumberInput
            | WidgetKind::ProgressBar
            | WidgetKind::Tab
            | WidgetKind::NavItem
    );

    if is_text_widget {
        let mut caret = None;
        if matches!(node.kind, WidgetKind::TextInput | WidgetKind::NumberInput) {
            let value = state.text_for(&node.id).unwrap_or("");
            if value.is_empty() {
                caret_positions.insert(node.id.clone(), 0.0);
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
                    WidgetKind::Tab => {
                        let scale = text_scale(font_size, theme);
                        let tab_pad = part_padding(node, &["tab"], pad, sf);
                        let left = r.x + tab_pad + TAB_GAP_LP * scale * 0.5;
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            left,
                            top,
                            left,
                            r.y,
                            r.x + r.w - tab_pad - TAB_GAP_LP * scale * 0.5,
                            r.y + r.h,
                        )
                    }
                    WidgetKind::ProgressBar => {
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (r.x + pad, top, r.x + pad, r.y, r.x + r.w - pad, r.y + r.h)
                    }
                    WidgetKind::NavItem => {
                        let item_pad = part_padding(node, &["item"], pad, sf);
                        let top = r.y + ((r.h - line_height) * 0.5).max(0.0);
                        (
                            r.x + item_pad,
                            top,
                            r.x + item_pad,
                            r.y,
                            r.x + r.w - item_pad,
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
                } else if node.kind == WidgetKind::Checkbox && !placeholder {
                    part_text_color(
                        node,
                        state,
                        theme,
                        &["label"],
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
                } else {
                    text_color(node, state, theme, placeholder)
                };
                let align = if node.kind == WidgetKind::ProgressBar
                    && node.style.text.text_align.is_none()
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
                            TextBounds {
                                left: clip_rect.x as i32,
                                top: clip_rect.y as i32,
                                right: (clip_rect.x + clip_rect.w) as i32,
                                bottom: (clip_rect.y + clip_rect.h) as i32,
                            },
                            color,
                            align,
                            cache,
                            if placeholder { None } else { caret },
                            caret_positions,
                        );
                    }
                }
            }
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

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
}

fn text_font_size(node: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
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

fn text_line_height(font_size: f32, theme: &Theme, sf: f32) -> f32 {
    (font_size + 5.0 * sf).max((theme.font_size + 3.0) * sf)
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
    let base = if let Some(color) = state_visual.and_then(|visual| visual.foreground.as_ref()) {
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
        .and_then(|visual| visual.opacity)
        .or(node.style.visual.opacity)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    glyph_color([base[0], base[1], base[2], base[3] * opacity])
}

fn state_visual_for<'a>(node: &'a WidgetNode, state: &WidgetState) -> Option<&'a VisualStyle> {
    if state.is_disabled(&node.id) {
        Some(&node.style.disabled)
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        Some(&node.style.active)
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        Some(&node.style.hover)
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        Some(&node.style.focus)
    } else if state.checked.get(&node.id).copied().unwrap_or(false) {
        Some(&node.style.checked)
    } else {
        None
    }
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
    let state_map = if state.is_disabled(&node.id) {
        Some(&node.style.parts.disabled)
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        Some(&node.style.parts.active)
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        Some(&node.style.parts.hover)
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        Some(&node.style.parts.focus)
    } else {
        None
    };

    for part in parts {
        if let Some(color) = state_map
            .and_then(|map| map.get(*part))
            .and_then(|style| part_style_text_color(style, theme))
        {
            return color;
        }
    }
    if state.checked.get(&node.id).copied().unwrap_or(false) {
        for part in parts {
            if let Some(color) = node
                .style
                .parts
                .checked
                .get(*part)
                .and_then(|style| part_style_text_color(style, theme))
            {
                return color;
            }
        }
    }
    for part in parts {
        if let Some(color) = node
            .style
            .parts
            .parts
            .get(*part)
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
        .find_map(|part| node.style.parts.parts.get(*part).map(|style| &style.text))
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
        if let Some(padding) = node
            .style
            .parts
            .parts
            .get(*part)
            .and_then(|style| style.layout.padding)
        {
            return (padding.max(0.0) * sf).max(0.0);
        }
    }
    default
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
    caret_positions: &mut HashMap<String, f32>,
    out: &mut Vec<TextEntry>,
) {
    if node.kind == WidgetKind::Dropdown && state.open_dropdown.as_deref() == Some(node.id.as_str())
    {
        if let (Some(r), Some(items)) = (
            layout.rects.get(&node.id),
            state.dropdown_items.get(&node.id),
        ) {
            let font_size = text_font_size(node, theme, sf);
            let line_height = text_line_height(font_size, theme, sf);
            let font_family = node.style.text.font_family.as_ref();
            let font_weight = node.style.text.font_weight.unwrap_or(Weight::NORMAL.0);
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
    caret_positions: &mut HashMap<String, f32>,
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
    caret_positions: &mut HashMap<String, f32>,
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
    let line_height = text_line_height(font_size, theme, sf);
    let font_family = node.style.text.font_family.as_ref();
    let font_weight = node.style.text.font_weight.unwrap_or(Weight::NORMAL.0);
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
    cache: &mut TextBufferCache,
    caret_positions: &mut HashMap<String, f32>,
    out: &mut Vec<TextEntry>,
) {
    let Some((node, rect)) = tooltip_target(tree, layout, theme, state, sf) else {
        return;
    };
    let Some(text) = node.props.tooltip.as_deref() else {
        return;
    };
    let pad = theme.spacing * sf * 1.25;
    let font_size = (theme.font_size * sf).max(8.0 * sf);
    let line_height = text_line_height(font_size, theme, sf);
    let top = rect.y + pad;
    push_wrapped_text_entry(
        font_system,
        out,
        text,
        font_size,
        line_height,
        None,
        Weight::NORMAL.0,
        rect.x + pad,
        top,
        TextBounds {
            left: (rect.x + pad) as i32,
            top: rect.y as i32,
            right: (rect.x + rect.w - pad) as i32,
            bottom: (rect.y + rect.h) as i32,
        },
        glyph_color(theme.text),
        TextAlign::Left,
        cache,
        caret_positions,
    );
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
    caret_positions: &mut HashMap<String, f32>,
    out: &mut Vec<TextEntry>,
) {
    if node.kind == WidgetKind::DataFrameTable {
        if let (Some(r), Some(table_state)) = (layout.visible_rect(&node.id), state.table(&node.id))
        {
            if r.w > 0.0
                && r.h > 0.0
                && !is_obscured_by_overlay(
                    node,
                    &r,
                    open_dropdown,
                    dropdown_overlay,
                    menu_overlays,
                    tooltip_overlay,
                )
            {
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
                let header_line_height = text_line_height(header_font_size, theme, sf);
                let header_font_family = part_font_family(node, header_parts, font_family);
                let header_font_weight = part_font_weight(node, header_parts, font_weight);
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
                    clamp_text_bounds_bottom(index_header_bounds, header_bottom),
                    header_muted,
                    TextAlign::Left,
                    cache,
                    None,
                    caret_positions,
                );

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
                        clamp_text_bounds_bottom(bounds, header_bottom),
                        header_color,
                        TextAlign::Left,
                        cache,
                        None,
                        caret_positions,
                    );
                }

                for row_offset in 0..visible.row_count {
                    let row = visible.first_row + row_offset;
                    let Some((row_y, row_bottom)) = table::row_bounds(&r, metrics, row_offset)
                    else {
                        continue;
                    };
                    if dropdown_overlay.is_some_and(|overlay| {
                        rects_intersect(
                            Rect {
                                x: r.x,
                                y: row_y,
                                w: r.w,
                                h: row_bottom - row_y,
                            },
                            overlay,
                        )
                    }) {
                        continue;
                    }

                    let selected = table_state
                        .selected
                        .is_some_and(|(selected_row, _)| selected_row == row);
                    let row_parts: &[&str] = if selected {
                        &["row-selected", "row"]
                    } else {
                        &["row"]
                    };
                    let row_font_size = part_font_size(node, row_parts, font_size, sf);
                    let row_line_height = text_line_height(row_font_size, theme, sf);
                    let row_font_family = part_font_family(node, row_parts, font_family);
                    let row_font_weight = part_font_weight(node, row_parts, font_weight);
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

                    push_text_entry(
                        font_system,
                        out,
                        &row.to_string(),
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
                    );

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
                        );
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
        WidgetKind::TextInput | WidgetKind::NumberInput => {
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
    caret_positions: &mut HashMap<String, f32>,
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
        cache,
        caret,
        caret_positions,
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
    caret_positions: &mut HashMap<String, f32>,
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
        cache,
        None,
        caret_positions,
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
    cache: &mut TextBufferCache,
    caret: Option<(&str, usize)>,
    caret_positions: &mut HashMap<String, f32>,
) {
    if clip.right <= clip.left
        || clip.bottom <= clip.top
        || left >= clip.right as f32
        || top >= clip.bottom as f32
    {
        return;
    }

    let avail_w = (clip.right as f32 - left).max(1.0);
    let key = TextKey {
        text: text.to_string(),
        font_family: font_family_key(font_family),
        font_weight,
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
            buf.set_text(
                font_system,
                text,
                &Attrs::new()
                    .family(to_glyphon_family(font_family))
                    .weight(Weight(font_weight)),
                Shaping::Advanced,
                None,
            );
            buf.shape_until_scroll(font_system, false);
            buf
        });
    if let Some((id, cursor)) = caret {
        caret_positions.insert(id.to_string(), caret_x_for_buffer(&buf, text, cursor));
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

fn caret_x_for_buffer(buffer: &Buffer, text: &str, cursor: usize) -> f32 {
    let cursor = clamp_boundary(text, cursor);
    if cursor == 0 || text.is_empty() {
        return 0.0;
    }

    let mut last_x = 0.0;
    for run in buffer.layout_runs().filter(|run| run.line_i == 0) {
        for glyph in run.glyphs {
            let glyph_left = glyph.x;
            let glyph_right = glyph.x + glyph.w;
            last_x = glyph_right.max(last_x);

            if cursor == glyph.start {
                return glyph_left.max(0.0);
            }
            if cursor == glyph.end {
                return glyph_right.max(0.0);
            }
            if cursor > glyph.start && cursor < glyph.end {
                let before = text[glyph.start..cursor].chars().count() as f32;
                let total = text[glyph.start..glyph.end].chars().count().max(1) as f32;
                let t = (before / total).clamp(0.0, 1.0);
                return (glyph_left + glyph.w * t).max(0.0);
            }
        }
    }

    last_x.max(0.0)
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
}
