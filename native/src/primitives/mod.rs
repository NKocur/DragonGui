use std::{borrow::Cow, collections::HashMap};

use bytemuck::{Pod, Zeroable};

use crate::document::{WidgetKind, WidgetNode};
use crate::events::{NavigationItem, WidgetState};
use crate::layout::{LayoutResult, Rect};
use crate::overlays::{menu_popup_rect, tooltip_target};
use crate::style::{
    number_stepper_width_for_style, tabs_header_height_for_style, VisualStyle, BORDER_WIDTH_LP,
    CARET_WIDTH_LP, CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP, FOCUS_RING_LP, PANEL_ACCENT_WIDTH_LP,
    SLIDER_THUMB_WIDTH_LP, SLIDER_TRACK_HEIGHT_LP, SLIDER_TRACK_MARGIN_LP, TAB_ACTIVE_BAR_LP,
    TAB_GAP_LP, TAB_INACTIVE_BOTTOM_INSET_LP, TAB_TOP_INSET_LP,
};
use crate::table;
use crate::theme::Theme;

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
}

static RECT_ATTRS: [wgpu::VertexAttribute; 4] = [
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

fn rect_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RectInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &RECT_ATTRS,
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

pub struct PrimitivesRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_cap: u64,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: Vec<RectInstance>,
    pub rect_count: u32,
}

impl PrimitivesRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("prim-rect"),
            source: wgpu::ShaderSource::Wgsl(include_str!("rect.wgsl").into()),
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

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("prim-rect-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[rect_instance_layout()],
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
                format: wgpu::TextureFormat::Depth32Float,
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

        let initial_cap = (64 * std::mem::size_of::<RectInstance>()) as u64;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prim-vb"),
            size: initial_cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let renderer = Self {
            pipeline,
            vertex_buffer,
            vertex_cap: initial_cap,
            uniform_buffer,
            bind_group,
            instances: Vec::with_capacity(64),
            rect_count: 0,
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
        caret_positions: &HashMap<String, f32>,
    ) {
        self.instances.clear();
        emit_rects(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            caret_positions,
            &mut self.instances,
        );
        emit_dropdown_overlays(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            &mut self.instances,
        );
        emit_menu_overlays(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            &mut self.instances,
        );
        emit_tooltip_overlay(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            &mut self.instances,
        );

        self.rect_count = self.instances.len() as u32;
        if self.instances.is_empty() {
            return;
        }

        let size = (self.instances.len() * std::mem::size_of::<RectInstance>()) as u64;
        if size > self.vertex_cap {
            let cap = (size * 2).max(4096);
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("prim-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_cap = cap;
        }
        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );
    }

    /// Record draw calls into an active render pass.
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.rect_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..6, 0..self.rect_count);
    }
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
    }
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

fn inset_radii(radii: [f32; 4], inset: f32) -> [f32; 4] {
    radii.map(|radius| (radius - inset).max(0.0))
}

fn outset_radii(radii: [f32; 4], outset: f32) -> [f32; 4] {
    radii.map(|radius| (radius + outset).max(0.0))
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

fn with_alpha(mut color: [f32; 4], alpha: f32) -> [f32; 4] {
    color[3] = alpha.clamp(0.0, 1.0);
    color
}

fn darken(color: [f32; 4], t: f32) -> [f32; 4] {
    mix(color, [0.0, 0.0, 0.0, color[3]], t)
}

fn visual_for<'a>(node: &'a WidgetNode, state: &WidgetState) -> Cow<'a, VisualStyle> {
    let base = &node.style.visual;
    if state.is_disabled(&node.id) {
        Cow::Owned(base.merged(&node.style.disabled))
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        Cow::Owned(base.merged(&node.style.active))
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        Cow::Owned(base.merged(&node.style.hover))
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        Cow::Owned(base.merged(&node.style.focus))
    } else if state.checked.get(&node.id).copied().unwrap_or(false) {
        Cow::Owned(base.merged(&node.style.checked))
    } else {
        Cow::Borrowed(base)
    }
}

fn part_visual_for(node: &WidgetNode, state: &WidgetState, part: &str) -> VisualStyle {
    let mut visual = node
        .style
        .parts
        .parts
        .get(part)
        .map(|style| style.visual.clone())
        .unwrap_or_default();
    if state.checked.get(&node.id).copied().unwrap_or(false) {
        if let Some(checked) = node
            .style
            .parts
            .checked
            .get(part)
            .map(|style| &style.visual)
        {
            visual = visual.merged(checked);
        }
    }
    let pseudo = if state.is_disabled(&node.id) {
        node.style
            .parts
            .disabled
            .get(part)
            .map(|style| &style.visual)
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        node.style.parts.active.get(part).map(|style| &style.visual)
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        node.style.parts.hover.get(part).map(|style| &style.visual)
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        node.style.parts.focus.get(part).map(|style| &style.visual)
    } else {
        None
    };

    if let Some(pseudo) = pseudo {
        visual = visual.merged(pseudo);
    }
    visual
}

fn part_style_active_for_state(node: &WidgetNode, state: &WidgetState, part: &str) -> bool {
    if node.style.parts.parts.contains_key(part) {
        return true;
    }
    if state.checked.get(&node.id).copied().unwrap_or(false)
        && node.style.parts.checked.contains_key(part)
    {
        return true;
    }
    if state.is_disabled(&node.id) {
        node.style.parts.disabled.contains_key(part)
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        node.style.parts.active.contains_key(part)
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        node.style.parts.hover.contains_key(part)
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        node.style.parts.focus.contains_key(part)
    } else {
        false
    }
}

fn merged_part_visual_for(node: &WidgetNode, state: &WidgetState, parts: &[&str]) -> VisualStyle {
    let mut visual = VisualStyle::default();
    for part in parts {
        visual = visual.merged(&part_visual_for(node, state, part));
    }
    visual
}

fn resolve_color(color: &Option<crate::style::ColorRef>, theme: &Theme) -> Option<[f32; 4]> {
    color.as_ref().map(|c| c.resolve(theme))
}

fn apply_opacity(mut color: [f32; 4], opacity: Option<f32>) -> [f32; 4] {
    if let Some(opacity) = opacity {
        color[3] *= opacity.clamp(0.0, 1.0);
    }
    color
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

fn control_fill(node: &WidgetNode, theme: &Theme, state: &WidgetState) -> [f32; 4] {
    if state.is_disabled(&node.id) {
        mix(theme.surface_alt, theme.disabled, 0.28)
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        darken(theme.accent, 0.15)
    } else if state.hovered.as_deref() == Some(node.id.as_str())
        || state.focused.as_deref() == Some(node.id.as_str())
    {
        mix(theme.surface_alt, theme.accent, 0.20)
    } else {
        theme.surface_alt
    }
}

fn control_border(node: &WidgetNode, theme: &Theme, state: &WidgetState) -> [f32; 4] {
    if state.is_disabled(&node.id) {
        mix(theme.border, theme.disabled, 0.45)
    } else if state.focused.as_deref() == Some(node.id.as_str()) {
        theme.accent
    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
        darken(theme.accent, 0.08)
    } else if state.hovered.as_deref() == Some(node.id.as_str()) {
        mix(theme.border, theme.accent, 0.35)
    } else {
        theme.border
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
    out.push(inst_radii(rect, border, radii));
    out.push(inst_radii(
        inset_rect(rect, border_w),
        fill,
        inset_radii(radii, border_w),
    ));
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
        let ring = FOCUS_RING_LP * sf;
        out.push(inst_radii(
            [
                rect[0] - ring,
                rect[1] - ring,
                rect[2] + ring * 2.0,
                rect[3] + ring * 2.0,
            ],
            with_alpha(theme.focus, 0.60),
            outset_radii(radii, ring),
        ));
    }
}

fn emit_rects(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, f32>,
    out: &mut Vec<RectInstance>,
) {
    if node.kind == WidgetKind::Modal && !node.props.open.unwrap_or(false) {
        return;
    }
    if let Some(r) = layout.visible_rect(&node.id) {
        let [x, y, w, h] = [r.x, r.y, r.w, r.h];
        let visual = visual_for(node, state);
        let border_w = visual.border_width.unwrap_or(BORDER_WIDTH_LP).max(0.0) * sf;
        let radius_lp = visual.border_radius.unwrap_or(theme.radius).max(0.0);
        let radius = radius_lp * sf;
        let radii = visual_radii(&visual, radius_lp, sf);
        let styled_bg =
            resolve_color(&visual.background, theme).map(|c| apply_opacity(c, visual.opacity));
        let styled_border =
            resolve_color(&visual.border_color, theme).map(|c| apply_opacity(c, visual.opacity));
        let styled_accent =
            resolve_color(&visual.accent, theme).map(|c| apply_opacity(c, visual.opacity));
        match node.kind {
            WidgetKind::Panel => {
                let panel_radius_lp = visual.border_radius.unwrap_or(theme.radius * 0.5).max(0.0);
                let panel_radii = visual_radii(&visual, panel_radius_lp, sf);
                let panel_fill = styled_bg.unwrap_or(theme.surface);
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    panel_fill,
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

            WidgetKind::Modal => {
                let root = root_rect(layout).unwrap_or(Rect { x, y, w, h });
                out.push(inst(
                    [root.x, root.y, root.w, root.h],
                    [0.0, 0.0, 0.0, 0.52],
                    0.0,
                ));
                let shadow = 6.0 * sf;
                out.push(inst_radii(
                    [x + shadow, y + shadow, w, h],
                    [0.0, 0.0, 0.0, 0.35],
                    radii,
                ));
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    styled_bg.unwrap_or(theme.surface),
                    radii,
                    border_w,
                );
                let accent_h = (PANEL_ACCENT_WIDTH_LP * sf).max(border_w);
                out.push(inst(
                    [
                        x + border_w,
                        y + border_w,
                        (w - border_w * 2.0).max(1.0),
                        accent_h,
                    ],
                    styled_accent.unwrap_or(theme.accent),
                    0.0,
                ));
            }

            WidgetKind::Sidebar => {
                out.push(inst_radii(
                    [x, y, w, h],
                    styled_bg.unwrap_or(theme.surface),
                    radii,
                ));
                out.push(inst(
                    [x + w - border_w, y, border_w, h],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::StatusBar => {
                out.push(inst_radii(
                    [x, y, w, h],
                    styled_bg.unwrap_or(theme.surface),
                    radii,
                ));
                out.push(inst(
                    [x, y, w, border_w],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::MenuBar => {
                out.push(inst_radii(
                    [x, y, w, h],
                    styled_bg.unwrap_or(theme.surface),
                    radii,
                ));
                out.push(inst(
                    [x, y + h - border_w, w, border_w],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::Separator => {
                out.push(inst(
                    [x, y, w.max(border_w), h.max(border_w)],
                    styled_bg.or(styled_border).unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::Tabs => {
                let header_visual = part_visual_for(node, state, "header");
                let header_h = tabs_header_height_for_style(&node.style, theme, sf);
                let header_border_w = header_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(border_w);
                let header_radii = visual_radii_with_fallback(&header_visual, [0.0; 4], sf);
                out.push(inst_radii(
                    [x, y, w, header_h],
                    apply_opacity(
                        resolve_color(&header_visual.background, theme)
                            .or(styled_bg)
                            .unwrap_or(theme.surface),
                        header_visual.opacity,
                    ),
                    header_radii,
                ));
                if header_border_w > 0.0 {
                    out.push(inst(
                        [
                            x,
                            y + header_h - header_border_w,
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

            WidgetKind::Button | WidgetKind::Dropdown | WidgetKind::Menu => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let menu_open = node.kind == WidgetKind::Menu
                    && state.open_menu.as_deref() == Some(node.id.as_str());
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or_else(|| control_border(node, theme, state)),
                    styled_bg.unwrap_or_else(|| {
                        if menu_open {
                            mix(
                                theme.surface_alt,
                                styled_accent.unwrap_or(theme.accent),
                                0.24,
                            )
                        } else {
                            control_fill(node, theme, state)
                        }
                    }),
                    radii,
                    border_w,
                );
            }

            WidgetKind::Tab => {
                let active = state.is_active_tab(&node.id);
                let tab_visual = part_visual_for(node, state, "tab");
                let accent_visual = part_visual_for(node, state, "accent");
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
                let tab_radii = visual_radii_with_fallback(&tab_visual, [vr; 4], sf);
                emit_focus_ring_radii(node, theme, sf, state, [vx, vy, vw, vh], tab_radii, out);
                let fill = if active {
                    resolve_color(&tab_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| {
                            mix(
                                theme.surface_alt,
                                styled_accent.unwrap_or(theme.accent),
                                0.24,
                            )
                        })
                } else if state.hovered.as_deref() == Some(node.id.as_str())
                    || state.focused.as_deref() == Some(node.id.as_str())
                {
                    resolve_color(&tab_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| {
                            mix(
                                theme.surface_alt,
                                styled_accent.unwrap_or(theme.accent),
                                0.12,
                            )
                        })
                } else if state.is_disabled(&node.id) {
                    resolve_color(&tab_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.28))
                } else {
                    resolve_color(&tab_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or(theme.surface_alt)
                };
                let tab_border_w = tab_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(border_w);
                emit_bordered_rect_radii(
                    out,
                    [vx, vy, vw, vh],
                    resolve_color(&tab_visual.border_color, theme)
                        .or(styled_border)
                        .unwrap_or(if active {
                            styled_accent.unwrap_or(theme.accent)
                        } else {
                            theme.border
                        }),
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
                    let accent_rect = [
                        vx + tab_border_w,
                        y + h - bar_h,
                        (vw - 2.0 * tab_border_w).max(1.0),
                        bar_h,
                    ];
                    let accent_fill = apply_opacity(
                        resolve_color(&accent_visual.background, theme)
                            .or(resolve_color(&accent_visual.foreground, theme))
                            .unwrap_or_else(|| styled_accent.unwrap_or(theme.accent)),
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
            }

            WidgetKind::NavItem => {
                let active = state.is_active_nav_item(&node.id);
                let item_visual = part_visual_for(node, state, "item");
                let accent_visual = part_visual_for(node, state, "accent");
                let item_radii = visual_radii_with_fallback(&item_visual, radii, sf);
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], item_radii, out);
                let fill = if active {
                    resolve_color(&item_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| {
                            mix(
                                theme.surface_alt,
                                styled_accent.unwrap_or(theme.accent),
                                0.20,
                            )
                        })
                } else {
                    resolve_color(&item_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or_else(|| control_fill(node, theme, state))
                };
                let item_border_w = item_visual
                    .border_width
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(0.0);
                let item_fill = apply_opacity(fill, item_visual.opacity);
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
                if active {
                    let bar_w = node
                        .style
                        .parts
                        .parts
                        .get("accent")
                        .and_then(|part| part.layout.width)
                        .map(|width| (width.max(1.0) * sf).max(1.0))
                        .unwrap_or(PANEL_ACCENT_WIDTH_LP * sf);
                    let accent_rect = [x, y, bar_w.min(w.max(1.0)), h];
                    let accent_fill = apply_opacity(
                        resolve_color(&accent_visual.background, theme)
                            .or(resolve_color(&accent_visual.foreground, theme))
                            .unwrap_or_else(|| styled_accent.unwrap_or(theme.accent)),
                        accent_visual.opacity,
                    );
                    let accent_radii =
                        visual_radii_with_fallback(&accent_visual, [bar_w * 0.5; 4], sf);
                    out.push(inst_radii(accent_rect, accent_fill, accent_radii));
                }
            }

            WidgetKind::TextInput => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let fill = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else if state.hovered.as_deref() == Some(node.id.as_str())
                    || state.focused.as_deref() == Some(node.id.as_str())
                {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.70))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55))
                };
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or_else(|| control_border(node, theme, state)),
                    fill,
                    radii,
                    border_w,
                );
                if state.focused.as_deref() == Some(node.id.as_str())
                    && !state.is_disabled(&node.id)
                {
                    let pad = theme.spacing * sf;
                    let text_w = (w - pad * 2.0).max(1.0);
                    let caret_x =
                        caret_x_for_node(x + pad, text_w, &node.id, state, caret_positions);
                    let caret_font_size = node.style.text.font_size.unwrap_or(theme.font_size) * sf;
                    let caret_h = (caret_font_size + 5.0 * sf).min((h - border_w * 2.0).max(1.0));
                    out.push(inst(
                        [
                            caret_x,
                            y + (h - caret_h) * 0.5,
                            CARET_WIDTH_LP * sf,
                            caret_h,
                        ],
                        theme.focus,
                        0.0,
                    ));
                }
            }

            WidgetKind::NumberInput => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let invalid = state.number_is_invalid(&node.id);
                let fill = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else if state.hovered.as_deref() == Some(node.id.as_str())
                    || state.focused.as_deref() == Some(node.id.as_str())
                {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.70))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55))
                };
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    if invalid {
                        theme.danger
                    } else {
                        styled_border.unwrap_or_else(|| control_border(node, theme, state))
                    },
                    fill,
                    radii,
                    border_w,
                );
                let step_w = number_stepper_width_for_style(&node.style, w, sf);
                let step_x = x + w - step_w;
                let step_fill = if state.is_disabled(&node.id) {
                    mix(theme.surface_alt, theme.disabled, 0.30)
                } else if state.hovered.as_deref() == Some(node.id.as_str())
                    || state.focused.as_deref() == Some(node.id.as_str())
                {
                    mix(
                        theme.surface_alt,
                        styled_accent.unwrap_or(theme.accent),
                        0.16,
                    )
                } else {
                    theme.surface_alt
                };
                let stepper_visual = part_visual_for(node, state, "stepper");
                let stepper_up_visual =
                    stepper_visual.merged(&part_visual_for(node, state, "stepper-up"));
                let stepper_down_visual =
                    stepper_visual.merged(&part_visual_for(node, state, "stepper-down"));
                let divider_visual = part_visual_for(node, state, "stepper-divider");
                let step_up_fill = resolve_color(&stepper_up_visual.background, theme)
                    .map(|color| apply_opacity(color, stepper_up_visual.opacity))
                    .unwrap_or(step_fill);
                let step_down_fill = resolve_color(&stepper_down_visual.background, theme)
                    .map(|color| apply_opacity(color, stepper_down_visual.opacity))
                    .unwrap_or(step_fill);
                let divider_color = resolve_color(&divider_visual.background, theme)
                    .or_else(|| resolve_color(&divider_visual.border_color, theme))
                    .map(|color| apply_opacity(color, divider_visual.opacity))
                    .unwrap_or(theme.border);
                let step_up_radii = visual_radii_with_fallback(
                    &stepper_up_visual,
                    [0.0, (radii[1] - border_w).max(0.0), 0.0, 0.0],
                    sf,
                );
                let step_down_radii = visual_radii_with_fallback(
                    &stepper_down_visual,
                    [0.0, 0.0, (radii[2] - border_w).max(0.0), 0.0],
                    sf,
                );
                out.push(inst(
                    [step_x, y + border_w, border_w, h - border_w * 2.0],
                    divider_color,
                    0.0,
                ));
                out.push(inst_radii(
                    [
                        step_x + border_w,
                        y + border_w,
                        (step_w - border_w).max(1.0),
                        (h * 0.5 - border_w).max(1.0),
                    ],
                    step_up_fill,
                    step_up_radii,
                ));
                out.push(inst_radii(
                    [
                        step_x + border_w,
                        y + h * 0.5,
                        (step_w - border_w).max(1.0),
                        (h * 0.5 - border_w).max(1.0),
                    ],
                    step_down_fill,
                    step_down_radii,
                ));
                out.push(inst(
                    [step_x, y + h * 0.5, step_w, border_w],
                    divider_color,
                    0.0,
                ));
                if state.focused.as_deref() == Some(node.id.as_str())
                    && !state.is_disabled(&node.id)
                {
                    let pad = theme.spacing * sf;
                    let text_w = (w - step_w - pad * 2.0).max(1.0);
                    let caret_x =
                        caret_x_for_node(x + pad, text_w, &node.id, state, caret_positions);
                    let caret_font_size = node.style.text.font_size.unwrap_or(theme.font_size) * sf;
                    let caret_h = (caret_font_size + 5.0 * sf).min((h - border_w * 2.0).max(1.0));
                    out.push(inst(
                        [
                            caret_x,
                            y + (h - caret_h) * 0.5,
                            CARET_WIDTH_LP * sf,
                            caret_h,
                        ],
                        if invalid { theme.danger } else { theme.focus },
                        0.0,
                    ));
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
                if !state.is_disabled(&node.id)
                    && (state.hovered.as_deref() == Some(node.id.as_str())
                        || state.pressed.as_deref() == Some(node.id.as_str())
                        || state.focused.as_deref() == Some(node.id.as_str()))
                    || row_visual.background.is_some()
                {
                    let fallback_row_fill = if state.pressed.as_deref() == Some(node.id.as_str()) {
                        with_alpha(darken(styled_accent.unwrap_or(theme.accent), 0.15), 0.20)
                    } else {
                        with_alpha(mix(theme.surface_alt, theme.accent, 0.20), 0.35)
                    };
                    let row_fill = resolve_color(&row_visual.background, theme)
                        .map(|color| apply_opacity(color, row_visual.opacity))
                        .unwrap_or(fallback_row_fill);
                    out.push(inst_radii(
                        [x, y, w, h],
                        row_fill,
                        visual_radii_with_fallback(&row_visual, radii, sf),
                    ));
                }
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let box_visual = part_visual_for(node, state, "box");
                let default_fill = if checked {
                    if disabled {
                        theme.disabled
                    } else if state.pressed.as_deref() == Some(node.id.as_str()) {
                        darken(styled_accent.unwrap_or(theme.accent), 0.15)
                    } else {
                        styled_accent.unwrap_or(theme.accent)
                    }
                } else {
                    styled_bg.unwrap_or_else(|| {
                        mix(theme.surface, control_fill(node, theme, state), 0.55)
                    })
                };
                let default_border = if checked {
                    if disabled {
                        theme.disabled
                    } else {
                        styled_border.unwrap_or(styled_accent.unwrap_or(theme.accent))
                    }
                } else {
                    styled_border.unwrap_or_else(|| control_border(node, theme, state))
                };
                let box_border_w = box_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
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
                    let default_marker_color = if disabled {
                        mix(theme.surface_alt, theme.disabled, 0.35)
                    } else {
                        theme.text
                    };
                    let marker_color = resolve_color(&indicator_visual.background, theme)
                        .or_else(|| resolve_color(&indicator_visual.foreground, theme))
                        .map(|color| apply_opacity(color, indicator_visual.opacity))
                        .unwrap_or(default_marker_color);
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

            WidgetKind::ProgressBar => {
                let track_visual = part_visual_for(node, state, "track");
                let fill_visual = part_visual_for(node, state, "fill");
                let default_track_fill = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.60))
                };
                let track_fill = resolve_color(&track_visual.background, theme)
                    .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)))
                    .unwrap_or(default_track_fill);
                let track_border_w = track_visual
                    .border_width
                    .map(|width| width.max(0.0) * sf)
                    .unwrap_or(border_w);
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_color(&track_visual.border_color, theme)
                        .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)))
                        .unwrap_or_else(|| styled_border.unwrap_or(theme.border)),
                    track_fill,
                    visual_radii_with_fallback(&track_visual, radii, sf),
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
                        .unwrap_or(if state.is_disabled(&node.id) {
                            theme.disabled
                        } else {
                            styled_accent.unwrap_or(theme.accent)
                        });
                    out.push(inst_radii(
                        [inner[0], fill_y, fill_w, fill_h],
                        fill_color,
                        visual_radii_with_fallback(&fill_visual, [fill_w.min(fill_h) * 0.5; 4], sf),
                    ));
                }
            }

            WidgetKind::Image => {
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    styled_bg.unwrap_or(theme.surface_alt),
                    radii,
                    border_w,
                );
            }

            WidgetKind::Slider => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let track_visual = part_visual_for(node, state, "track");
                let fill_visual = part_visual_for(node, state, "fill");
                let thumb_visual = part_visual_for(node, state, "thumb");
                let track_color = resolve_color(&track_visual.background, theme)
                    .map(|color| apply_opacity(color, track_visual.opacity.or(visual.opacity)))
                    .or_else(|| {
                        resolve_color(&visual.track_color, theme)
                            .map(|color| apply_opacity(color, visual.opacity))
                    })
                    .unwrap_or(theme.border);
                let thumb_color = resolve_color(&thumb_visual.background, theme)
                    .or_else(|| resolve_color(&thumb_visual.foreground, theme))
                    .map(|color| apply_opacity(color, thumb_visual.opacity.or(visual.opacity)))
                    .or_else(|| {
                        resolve_color(&visual.thumb_color, theme)
                            .map(|color| apply_opacity(color, visual.opacity))
                    })
                    .unwrap_or_else(|| styled_accent.unwrap_or(theme.accent));
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
                        .unwrap_or(if state.is_disabled(&node.id) {
                            theme.disabled
                        } else {
                            thumb_color
                        });
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
                        .unwrap_or_else(|| {
                            styled_border.unwrap_or_else(|| control_border(node, theme, state))
                        }),
                    if state.is_disabled(&node.id) {
                        theme.disabled
                    } else {
                        thumb_color
                    },
                    visual_radii_with_fallback(&thumb_visual, [thumb_w.min(thumb_h) * 0.5; 4], sf),
                    thumb_border_w,
                );
            }

            WidgetKind::Scatter3D => {
                let border = styled_border.unwrap_or(theme.border);
                out.push(inst([x, y, w, border_w], border, 0.0));
                out.push(inst([x, y + h - border_w, w, border_w], border, 0.0));
                out.push(inst([x, y, border_w, h], border, 0.0));
                out.push(inst([x + w - border_w, y, border_w, h], border, 0.0));
            }

            WidgetKind::DataFrameTable => {
                let header_visual = part_visual_for(node, state, "header");
                let row_visual = part_visual_for(node, state, "row");
                let selected_row_visual = part_visual_for(node, state, "row-selected");
                let grid_visual = part_visual_for(node, state, "grid-line");
                let grid_color = resolve_color(&grid_visual.background, theme)
                    .or_else(|| resolve_color(&grid_visual.foreground, theme))
                    .or_else(|| resolve_color(&grid_visual.border_color, theme))
                    .or(styled_border)
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
                    .unwrap_or(border_w)
                    .max(1.0);
                let table_rect = [x, y, w, h];
                let table_radii = radii;
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                out.push(inst_radii(
                    table_rect,
                    styled_bg.unwrap_or(theme.surface),
                    radii,
                ));
                if let Some(table_state) = state.table(&node.id) {
                    let metrics = table::metrics_for_node(node, theme, sf);
                    let visible = table::visible(table_state, &r, metrics);
                    let table_right = x + w;
                    let table_bottom = y + h;
                    let header_h = metrics.header_h.min(h);
                    let header_fill = resolve_color(&header_visual.background, theme)
                        .or_else(|| resolve_color(&header_visual.foreground, theme))
                        .map(|color| apply_opacity(color, header_visual.opacity))
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
                        let Some((col_x, _)) = table::column_bounds(&r, metrics, col_offset) else {
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

                    let row_fill = resolve_color(&row_visual.background, theme)
                        .or_else(|| resolve_color(&row_visual.foreground, theme))
                        .map(|color| apply_opacity(color, row_visual.opacity));
                    let selected_row_fill = resolve_color(&selected_row_visual.background, theme)
                        .or_else(|| resolve_color(&selected_row_visual.foreground, theme))
                        .map(|color| apply_opacity(color, selected_row_visual.opacity));

                    for row_offset in 0..visible.row_count {
                        let row = visible.first_row + row_offset;
                        let Some((row_y, row_bottom)) = table::row_bounds(&r, metrics, row_offset)
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
                            if let Some((col_x, col_right)) =
                                table::column_bounds(&r, metrics, selected_col - visible.first_col)
                            {
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
                }
                let border_color = styled_border.unwrap_or(grid_color);
                push_masked_rect(
                    out,
                    table_rect,
                    border_color,
                    table_radii,
                    [x, y, w, border_w],
                );
                push_masked_rect(
                    out,
                    table_rect,
                    border_color,
                    table_radii,
                    [x, y + h - border_w, w, border_w],
                );
                push_masked_rect(
                    out,
                    table_rect,
                    border_color,
                    table_radii,
                    [x, y, border_w, h],
                );
                push_masked_rect(
                    out,
                    table_rect,
                    border_color,
                    table_radii,
                    [x + w - border_w, y, border_w, h],
                );
            }

            WidgetKind::Window
            | WidgetKind::HLayout
            | WidgetKind::VLayout
            | WidgetKind::Pages
            | WidgetKind::Page
            | WidgetKind::Spacer
            | WidgetKind::Label
            | WidgetKind::ContextMenu
            | WidgetKind::MenuItem
            | WidgetKind::Unknown => {}
        }
    }

    for child in &node.children {
        emit_rects(child, layout, theme, sf, state, caret_positions, out);
    }
}

fn caret_x_for_node(
    left: f32,
    text_width: f32,
    id: &str,
    state: &WidgetState,
    caret_positions: &HashMap<String, f32>,
) -> f32 {
    left + caret_positions
        .get(id)
        .copied()
        .unwrap_or_else(|| text_width * state.caret_t(id))
        .clamp(0.0, text_width)
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
                emit_menu_popup(rect, items, theme, sf, state, out);
            }
        }
    }
    if let Some(menu_id) = state.open_context_menu.as_deref() {
        if let Some(rect) = menu_popup_rect(tree, layout, state, theme, sf, menu_id) {
            if let Some(items) = state.menu_items.get(menu_id) {
                emit_menu_popup(rect, items, theme, sf, state, out);
            }
        }
    }
}

fn emit_menu_popup(
    rect: Rect,
    items: &[NavigationItem],
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    out: &mut Vec<RectInstance>,
) {
    if items.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }
    let radius = theme.radius * sf;
    let radii = [radius; 4];
    let border_w = BORDER_WIDTH_LP * sf;
    let row_h = theme.control_height() * sf;
    let popup_rect = [rect.x, rect.y, rect.w, rect.h];
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
    emit_bordered_rect_radii(
        out,
        popup_rect,
        mix(theme.border, theme.accent, 0.18),
        theme.surface,
        radii,
        border_w,
    );
    for (idx, item) in items.iter().enumerate() {
        let y = rect.y + idx as f32 * row_h;
        let disabled = item.disabled || state.is_disabled(&item.id);
        let color = if disabled {
            mix(theme.surface, theme.disabled, 0.18)
        } else if state.hovered.as_deref() == Some(item.id.as_str()) {
            mix(theme.surface_alt, theme.accent, 0.24)
        } else {
            theme.surface_alt
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
                row_h - border_w,
            ],
        );
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
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    out: &mut Vec<RectInstance>,
) {
    if node.kind == WidgetKind::Dropdown && state.open_dropdown.as_deref() == Some(node.id.as_str())
    {
        if let (Some(r), Some(items)) = (
            layout.rects.get(&node.id),
            state.dropdown_items.get(&node.id),
        ) {
            let row_h = theme.control_height() * sf;
            let menu_h = row_h * items.len() as f32;
            let menu_visual = part_visual_for(node, state, "menu");
            let menu_radius_lp = menu_visual.border_radius.unwrap_or(theme.radius).max(0.0);
            let menu_radii = visual_radii(&menu_visual, menu_radius_lp, sf);
            let border_w = menu_visual.border_width.unwrap_or(BORDER_WIDTH_LP).max(0.0) * sf;
            let menu_rect = [r.x, r.y + r.h, r.w, menu_h];
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
            emit_bordered_rect_radii(
                out,
                menu_rect,
                resolve_color(&menu_visual.border_color, theme)
                    .map(|color| apply_opacity(color, menu_visual.opacity))
                    .unwrap_or_else(|| mix(theme.border, theme.accent, 0.18)),
                resolve_color(&menu_visual.background, theme)
                    .map(|color| apply_opacity(color, menu_visual.opacity))
                    .unwrap_or(theme.surface),
                menu_radii,
                border_w,
            );
            let selected = state.dropdown_index.get(&node.id).copied().unwrap_or(0);
            let hovered = state
                .dropdown_hover
                .as_ref()
                .filter(|(id, _)| id == &node.id)
                .map(|(_, idx)| *idx);
            for idx in 0..items.len() {
                let y = r.y + r.h + idx as f32 * row_h;
                let row_visual = if Some(idx) == hovered && idx == selected {
                    merged_part_visual_for(node, state, &["item", "item-selected", "item-hover"])
                } else if Some(idx) == hovered {
                    merged_part_visual_for(node, state, &["item", "item-hover"])
                } else if idx == selected {
                    merged_part_visual_for(node, state, &["item", "item-selected"])
                } else {
                    part_visual_for(node, state, "item")
                };
                let color = resolve_color(&row_visual.background, theme)
                    .map(|color| apply_opacity(color, row_visual.opacity))
                    .unwrap_or_else(|| {
                        if Some(idx) == hovered && idx == selected {
                            mix(theme.surface_alt, theme.accent, 0.42)
                        } else if Some(idx) == hovered {
                            mix(theme.surface_alt, theme.accent, 0.24)
                        } else if idx == selected {
                            mix(theme.surface_alt, theme.accent, 0.28)
                        } else {
                            theme.surface_alt
                        }
                    });
                push_masked_rect(
                    out,
                    menu_rect,
                    color,
                    menu_radii,
                    [
                        r.x + border_w,
                        y + border_w,
                        r.w - border_w * 2.0,
                        row_h - border_w,
                    ],
                );
            }
        }
    }

    for child in &node.children {
        emit_dropdown_overlays(child, layout, theme, sf, state, out);
    }
}

fn emit_tooltip_overlay(
    tree: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    out: &mut Vec<RectInstance>,
) {
    let Some((_node, rect)) = tooltip_target(tree, layout, theme, state, sf) else {
        return;
    };
    let border_w = BORDER_WIDTH_LP * sf;
    let radius = (theme.radius * sf).max(0.0);
    let shadow = 4.0 * sf;
    out.push(inst(
        [rect.x + shadow, rect.y + shadow, rect.w, rect.h],
        [0.0, 0.0, 0.0, 0.36],
        radius,
    ));
    emit_bordered_rect(
        out,
        [rect.x, rect.y, rect.w, rect.h],
        mix(theme.border, theme.accent, 0.18),
        theme.surface_alt,
        radius,
        border_w,
    );
}
