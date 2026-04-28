use std::{borrow::Cow, collections::HashMap};

use bytemuck::{Pod, Zeroable};

use crate::css_style::{computed_style_for_virtual_element, StylesheetStore};
use crate::document::{WidgetKind, WidgetNode};
use crate::events::{NavigationItem, WidgetState};
use crate::layout::{LayoutResult, Rect};
use crate::overlays::{menu_popup_rect, rich_tooltip_target, tooltip_target};
use crate::style::{
    badge_height_for_style, badge_width_for_text, collapsible_header_height_for_style,
    merged_part_visual_for_state as style_merged_part_visual_for_state,
    number_stepper_width_for_style,
    part_style_active_for_state as style_part_style_active_for_state,
    part_visual_for_state as style_part_visual_for_state, tabs_header_height_for_style,
    uniform_layout_padding, BackgroundPaint, NodeStyle, VisualStyle, BORDER_WIDTH_LP,
    CARET_WIDTH_LP, CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP, FOCUS_RING_LP, PANEL_ACCENT_WIDTH_LP,
    SLIDER_THUMB_WIDTH_LP, SLIDER_TRACK_HEIGHT_LP, SLIDER_TRACK_MARGIN_LP, TAB_ACTIVE_BAR_LP,
    TAB_GAP_LP, TAB_INACTIVE_BOTTOM_INSET_LP, TAB_TOP_INSET_LP,
};
use crate::table;
use crate::theme::Theme;
use crate::toast::{toast_colors, toast_rect, toast_stack_index, ToastOverlay};

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
    /// x: edge softness, y: shape inset, z: shadow mode flag, w: reserved.
    pub params: [f32; 4],
    /// Secondary RGBA colour for gradient paints.
    pub color2: [f32; 4],
    /// x: paint kind, y/z: linear-gradient direction, w: reserved.
    pub paint: [f32; 4],
}

static RECT_ATTRS: [wgpu::VertexAttribute; 7] = [
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
        caret_positions: &HashMap<String, [f32; 2]>,
        toasts: &[ToastOverlay],
        stylesheets: &StylesheetStore,
        window_w: f32,
        window_h: f32,
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
            caret_positions,
            stylesheets,
            &mut self.instances,
        );
        emit_toast_overlays(
            toasts,
            theme,
            scale_factor,
            stylesheets,
            window_w,
            window_h,
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
        params: [1.0, 0.0, 0.0, 0.0],
        color2: color,
        paint: [0.0, 0.0, 0.0, 0.0],
    }
}

fn inst_shadow(rect: [f32; 4], color: [f32; 4], radii: [f32; 4], blur: f32) -> RectInstance {
    RectInstance {
        rect,
        color,
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [blur.max(1.0), blur.max(0.0), 1.0, 0.0],
        color2: color,
        paint: [0.0, 0.0, 0.0, 0.0],
    }
}

fn inst_linear_gradient(
    rect: [f32; 4],
    start: [f32; 4],
    end: [f32; 4],
    radii: [f32; 4],
    angle_deg: f32,
) -> RectInstance {
    let angle = angle_deg.to_radians();
    let dir = [angle.sin(), -angle.cos()];
    RectInstance {
        rect,
        color: start,
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [1.0, 0.0, 0.0, 0.0],
        color2: end,
        paint: [1.0, dir[0], dir[1], 0.0],
    }
}

fn inst_radial_gradient(
    rect: [f32; 4],
    center: [f32; 4],
    edge: [f32; 4],
    radii: [f32; 4],
) -> RectInstance {
    RectInstance {
        rect,
        color: center,
        radii,
        clip: [-1.0, -1.0, rect[2] + 1.0, rect[3] + 1.0],
        params: [1.0, 0.0, 0.0, 0.0],
        color2: edge,
        paint: [2.0, 0.0, 0.0, 0.0],
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
    let mut visual = base.clone();
    let mut changed = false;
    merge_semantic_visual_states(&mut visual, node, state, &mut changed);
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
    if changed {
        Cow::Owned(visual)
    } else {
        Cow::Borrowed(base)
    }
}

fn merge_semantic_visual_states(
    visual: &mut VisualStyle,
    node: &WidgetNode,
    state: &WidgetState,
    changed: &mut bool,
) {
    if state.checked.get(&node.id).copied().unwrap_or(false) {
        *visual = visual.merged(&node.style.checked);
        *changed = true;
    }
    if node_is_open(node, state) {
        *visual = visual.merged(&node.style.open);
        *changed = true;
    }
    if state.is_expanded_widget(&node.id) {
        *visual = visual.merged(&node.style.expanded);
        *changed = true;
    }
    if state.is_collapsed_widget(&node.id) {
        *visual = visual.merged(&node.style.collapsed);
        *changed = true;
    }
    if state.is_selected_widget(&node.id) {
        *visual = visual.merged(&node.style.selected);
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

fn resolve_color(color: &Option<crate::style::ColorRef>, theme: &Theme) -> Option<[f32; 4]> {
    color.as_ref().map(|c| c.resolve(theme))
}

#[derive(Debug, Clone)]
enum FillPaint {
    Solid([f32; 4]),
    LinearGradient {
        start: [f32; 4],
        end: [f32; 4],
        angle_deg: f32,
    },
    RadialGradient {
        center: [f32; 4],
        edge: [f32; 4],
    },
}

fn apply_opacity(mut color: [f32; 4], opacity: Option<f32>) -> [f32; 4] {
    if let Some(opacity) = opacity {
        color[3] *= opacity.clamp(0.0, 1.0);
    }
    color
}

fn resolve_overlay_opacity(style: &NodeStyle, base_opacity: f32) -> f32 {
    (base_opacity * style.visual.opacity.unwrap_or(1.0)).clamp(0.0, 1.0)
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

fn resolve_background_paint(visual: &VisualStyle, theme: &Theme, fallback: [f32; 4]) -> FillPaint {
    match &visual.background_paint {
        Some(BackgroundPaint::Color(color)) => {
            FillPaint::Solid(apply_opacity(color.resolve(theme), visual.opacity))
        }
        Some(BackgroundPaint::LinearGradient(gradient)) if gradient.stops.len() >= 2 => {
            let first = gradient.stops.first().expect("checked length");
            let last = gradient.stops.last().expect("checked length");
            FillPaint::LinearGradient {
                start: apply_opacity(first.color.resolve(theme), visual.opacity),
                end: apply_opacity(last.color.resolve(theme), visual.opacity),
                angle_deg: gradient.angle_deg,
            }
        }
        Some(BackgroundPaint::RadialGradient(gradient)) if gradient.stops.len() >= 2 => {
            let first = gradient.stops.first().expect("checked length");
            let last = gradient.stops.last().expect("checked length");
            FillPaint::RadialGradient {
                center: apply_opacity(first.color.resolve(theme), visual.opacity),
                edge: apply_opacity(last.color.resolve(theme), visual.opacity),
            }
        }
        _ => FillPaint::Solid(
            resolve_color(&visual.background, theme)
                .map(|color| apply_opacity(color, visual.opacity))
                .unwrap_or(fallback),
        ),
    }
}

fn emit_paint_rect_radii(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    paint: FillPaint,
    radii: [f32; 4],
) {
    match paint {
        FillPaint::Solid(color) => out.push(inst_radii(rect, color, radii)),
        FillPaint::LinearGradient {
            start,
            end,
            angle_deg,
        } => out.push(inst_linear_gradient(rect, start, end, radii, angle_deg)),
        FillPaint::RadialGradient { center, edge } => {
            out.push(inst_radial_gradient(rect, center, edge, radii))
        }
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

fn emit_bordered_paint_rect_radii(
    out: &mut Vec<RectInstance>,
    rect: [f32; 4],
    border: [f32; 4],
    fill: FillPaint,
    radii: [f32; 4],
    border_w: f32,
) {
    out.push(inst_radii(rect, border, radii));
    emit_paint_rect_radii(
        out,
        inset_rect(rect, border_w),
        fill,
        inset_radii(radii, border_w),
    );
}

fn emit_box_shadows(
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
        out.push(inst_shadow(
            cover_rect,
            color,
            outset_radii(radii, spread),
            blur,
        ));
    }
}

fn widget_supports_box_shadow(kind: WidgetKind) -> bool {
    !matches!(
        kind,
        WidgetKind::Window | WidgetKind::Modal | WidgetKind::Tooltip | WidgetKind::Spacer
    )
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
    caret_positions: &HashMap<String, [f32; 2]>,
    out: &mut Vec<RectInstance>,
) {
    if node.kind == WidgetKind::Tooltip {
        return;
    }
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
        if widget_supports_box_shadow(node.kind) {
            emit_box_shadows(out, [x, y, w, h], radii, &visual, theme, sf);
        }
        match node.kind {
            WidgetKind::Panel => {
                let panel_radius_lp = visual.border_radius.unwrap_or(theme.radius * 0.5).max(0.0);
                let panel_radii = visual_radii(&visual, panel_radius_lp, sf);
                let panel_fill = resolve_background_paint(&visual, theme, theme.surface);
                emit_bordered_paint_rect_radii(
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

            WidgetKind::Collapsible => {
                let expanded = state.is_expanded(&node.id);
                let header_visual = part_visual_for(node, state, "header");
                let body_visual = part_visual_for(node, state, "body");
                let header_h = collapsible_header_height_for_style(&node.style, theme, sf).min(h);
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, theme.surface)
                } else {
                    FillPaint::Solid(styled_bg.unwrap_or(theme.surface))
                };
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or_else(|| control_border(node, theme, state)),
                    fill,
                    radii,
                    border_w,
                );
                let header_fill = resolve_color(&header_visual.background, theme)
                    .map(|color| apply_opacity(color, header_visual.opacity))
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
                            .or(styled_border)
                            .unwrap_or(theme.border),
                        0.0,
                    ));
                }
                if part_style_active_for_state(node, state, "indicator") {
                    let indicator_visual = part_visual_for(node, state, "indicator");
                    if let Some(indicator_fill) = resolve_color(&indicator_visual.background, theme)
                        .map(|color| apply_opacity(color, indicator_visual.opacity))
                    {
                        let size = node
                            .style
                            .parts
                            .parts
                            .get("indicator")
                            .and_then(|part| part.layout.width)
                            .unwrap_or(16.0)
                            .max(1.0)
                            * sf;
                        out.push(inst_radii(
                            [
                                x + theme.spacing * sf,
                                y + (header_h - size) * 0.5,
                                size,
                                size,
                            ],
                            indicator_fill,
                            visual_radii_with_fallback(&indicator_visual, [size * 0.5; 4], sf),
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
                if visual.box_shadows.is_some() {
                    emit_box_shadows(out, [x, y, w, h], radii, &visual, theme, sf);
                } else {
                    let shadow = 6.0 * sf;
                    out.push(inst_radii(
                        [x + shadow, y + shadow, w, h],
                        [0.0, 0.0, 0.0, 0.35],
                        radii,
                    ));
                }
                let fill = resolve_background_paint(&visual, theme, theme.surface);
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    fill,
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
                emit_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_background_paint(&visual, theme, theme.surface),
                    radii,
                );
                out.push(inst(
                    [x + w - border_w, y, border_w, h],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::StatusBar => {
                emit_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_background_paint(&visual, theme, theme.surface),
                    radii,
                );
                out.push(inst(
                    [x, y, w, border_w],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::MenuBar => {
                emit_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    resolve_background_paint(&visual, theme, theme.surface),
                    radii,
                );
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
                let header_fallback = apply_opacity(
                    resolve_color(&header_visual.background, theme)
                        .or(styled_bg)
                        .unwrap_or(theme.surface),
                    header_visual.opacity,
                );
                let header_fill = if header_visual.background_paint.is_some() {
                    resolve_background_paint(&header_visual, theme, header_fallback)
                } else {
                    FillPaint::Solid(header_fallback)
                };
                emit_paint_rect_radii(out, [x, y, w, header_h], header_fill, header_radii);
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
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, control_fill(node, theme, state))
                } else {
                    FillPaint::Solid(styled_bg.unwrap_or_else(|| {
                        if menu_open {
                            mix(
                                theme.surface_alt,
                                styled_accent.unwrap_or(theme.accent),
                                0.24,
                            )
                        } else {
                            control_fill(node, theme, state)
                        }
                    }))
                };
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or_else(|| control_border(node, theme, state)),
                    fill,
                    radii,
                    border_w,
                );
                if node.kind == WidgetKind::Button {
                    if let Some(rect) =
                        badge_rect(node, [x, y, w, h], theme, sf, theme.spacing * sf)
                    {
                        emit_badge_pill(node, theme, sf, state, rect, out);
                    }
                }
            }

            WidgetKind::Badge | WidgetKind::Tag => {
                let fill_solid = styled_bg.unwrap_or_else(|| standalone_badge_fill(node, theme));
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, fill_solid)
                } else {
                    FillPaint::Solid(fill_solid)
                };
                let border_color = styled_border.unwrap_or_else(|| {
                    if node.kind == WidgetKind::Tag {
                        standalone_badge_level_color(node, theme)
                    } else {
                        fill_solid
                    }
                });
                let badge_border_w = if node.kind == WidgetKind::Tag {
                    border_w
                } else {
                    visual.border_width.unwrap_or(0.0).max(0.0) * sf
                };
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
                if let Some(rect) = badge_rect(node, [x, y, w, h], theme, sf, gap) {
                    emit_badge_pill(node, theme, sf, state, rect, out);
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
                if let Some(rect) = badge_rect(node, [x, y, w, h], theme, sf, theme.spacing * sf) {
                    emit_badge_pill(node, theme, sf, state, rect, out);
                }
            }

            WidgetKind::TextInput | WidgetKind::TextArea => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let fill_solid = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else if state.hovered.as_deref() == Some(node.id.as_str())
                    || state.focused.as_deref() == Some(node.id.as_str())
                {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.70))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55))
                };
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, fill_solid)
                } else {
                    FillPaint::Solid(fill_solid)
                };
                emit_bordered_paint_rect_radii(
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
                    let caret_xy =
                        caret_xy_for_node(x + pad, text_w, &node.id, state, caret_positions);
                    let caret_font_size = node.style.text.font_size.unwrap_or(theme.font_size) * sf;
                    let caret_h = (caret_font_size + 5.0 * sf).min((h - border_w * 2.0).max(1.0));
                    let caret_y = if node.kind == WidgetKind::TextArea {
                        y + pad + caret_xy[1]
                    } else {
                        y + (h - caret_h) * 0.5
                    };
                    let visible_caret = node.kind != WidgetKind::TextArea
                        || (caret_y < y + h - border_w && caret_y + caret_h > y + border_w);
                    if visible_caret {
                        let caret_y = if node.kind == WidgetKind::TextArea {
                            caret_y.clamp(y + border_w, y + h - border_w - caret_h)
                        } else {
                            caret_y
                        };
                        out.push(inst(
                            [caret_xy[0], caret_y, CARET_WIDTH_LP * sf, caret_h],
                            theme.focus,
                            0.0,
                        ));
                    }
                }
            }

            WidgetKind::NumberInput => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let invalid = state.number_is_invalid(&node.id);
                let field_visual = part_visual_for(node, state, "field");
                let fill_solid = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else if state.hovered.as_deref() == Some(node.id.as_str())
                    || state.focused.as_deref() == Some(node.id.as_str())
                {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.70))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55))
                };
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, fill_solid)
                } else {
                    FillPaint::Solid(fill_solid)
                };
                emit_bordered_paint_rect_radii(
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
                let field_rect = [
                    x + border_w,
                    y + border_w,
                    (step_x - x - border_w).max(1.0),
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
                    let field_radii = visual_radii_with_fallback(
                        &field_visual,
                        [
                            (radii[0] - border_w).max(0.0),
                            0.0,
                            0.0,
                            (radii[3] - border_w).max(0.0),
                        ],
                        sf,
                    );
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
                let stepper_divider_visual = part_visual_for(node, state, "stepper-divider");
                let divider_visual = if part_style_active_for_state(node, state, "divider") {
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
                    .unwrap_or(theme.border);
                let stepper_divider_color =
                    resolve_color(&stepper_divider_visual.background, theme)
                        .or_else(|| resolve_color(&stepper_divider_visual.border_color, theme))
                        .map(|color| apply_opacity(color, stepper_divider_visual.opacity))
                        .unwrap_or(divider_color);
                let divider_w = divider_visual
                    .border_width
                    .or_else(|| {
                        node.style
                            .parts
                            .parts
                            .get("divider")
                            .and_then(|part| part.layout.width)
                    })
                    .map(|width| (width.max(0.0) * sf).max(0.0))
                    .unwrap_or(border_w)
                    .max(1.0);
                let stepper_divider_h = stepper_divider_visual
                    .border_width
                    .or_else(|| {
                        node.style
                            .parts
                            .parts
                            .get("stepper-divider")
                            .and_then(|part| part.layout.height)
                    })
                    .map(|height| (height.max(0.0) * sf).max(0.0))
                    .unwrap_or(border_w)
                    .max(1.0);
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
                    [step_x, y + border_w, divider_w, h - border_w * 2.0],
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
                    [step_x, y + h * 0.5, step_w, stepper_divider_h],
                    stepper_divider_color,
                    0.0,
                ));
                if state.focused.as_deref() == Some(node.id.as_str())
                    && !state.is_disabled(&node.id)
                {
                    let pad = theme.spacing * sf;
                    let text_w = (w - step_w - pad * 2.0).max(1.0);
                    let caret_x =
                        caret_xy_for_node(x + pad, text_w, &node.id, state, caret_positions)[0];
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
                emit_bordered_paint_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    resolve_background_paint(&visual, theme, theme.surface_alt),
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
            | WidgetKind::Tooltip
            | WidgetKind::Toast
            | WidgetKind::Unknown => {}
        }
    }

    for child in &node.children {
        emit_rects(child, layout, theme, sf, state, caret_positions, out);
    }
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

fn standalone_badge_fill(node: &WidgetNode, theme: &Theme) -> [f32; 4] {
    let semantic = standalone_badge_level_color(node, theme);
    if node.kind == WidgetKind::Tag {
        mix(theme.surface_alt, semantic, 0.22)
    } else {
        semantic
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
    let badge_w = badge_width_for_text(&node.style, badge, theme, sf);
    let badge_h = badge_height_for_style(&node.style, theme, sf).min((rect[3] - 4.0 * sf).max(1.0));
    let x = rect[0] + rect[2] - right_inset - badge_w;
    let y = rect[1] + (rect[3] - badge_h) * 0.5;
    if x <= rect[0] || badge_w <= 0.0 || badge_h <= 0.0 {
        return None;
    }
    Some([x, y, badge_w, badge_h])
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
            if menu_visual.box_shadows.is_some() {
                emit_box_shadows(out, menu_rect, menu_radii, &menu_visual, theme, sf);
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
    caret_positions: &HashMap<String, [f32; 2]>,
    stylesheets: &StylesheetStore,
    out: &mut Vec<RectInstance>,
) {
    if let Some((node, rect)) = rich_tooltip_target(tree, layout, state) {
        emit_tooltip_surface(node, rect, theme, sf, state, out);
        for child in &node.children {
            emit_rects(child, layout, theme, sf, state, caret_positions, out);
        }
        return;
    }
    let Some((_node, rect)) = tooltip_target(tree, layout, theme, state, sf) else {
        return;
    };
    let style = computed_style_for_virtual_element(
        WidgetKind::Tooltip,
        "__dg_static_tooltip",
        &["static"],
        stylesheets,
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
    let border_w = BORDER_WIDTH_LP * sf;
    let visual = visual_for(node, state);
    let radius_lp = visual.border_radius.unwrap_or(theme.radius).max(0.0);
    let radius = radius_lp * sf;
    if visual.box_shadows.is_some() {
        emit_box_shadows(
            out,
            [rect.x, rect.y, rect.w, rect.h],
            [radius; 4],
            &visual,
            theme,
            sf,
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
            .unwrap_or_else(|| mix(theme.border, theme.accent, 0.18)),
        resolve_color(&visual.background, theme)
            .map(|color| apply_opacity(color, visual.opacity))
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
    let border_w = BORDER_WIDTH_LP * sf;
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
            mix(theme.border, theme.accent, 0.18),
            opacity,
        ),
        overlay_color(
            &style.visual.background,
            theme,
            with_alpha(theme.surface_alt, 1.0),
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
    window_w: f32,
    window_h: f32,
    out: &mut Vec<RectInstance>,
) {
    let border_w = BORDER_WIDTH_LP * sf;
    let mut stack_counts = [0usize; 4];
    for toast in toasts {
        let classes = [toast.level.as_str()];
        let style = computed_style_for_virtual_element(
            WidgetKind::Toast,
            toast.id.as_str(),
            &classes,
            stylesheets,
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
    use crate::document::NodeProps;
    use crate::style::{
        BackgroundPaint, BoxShadow, ColorRef, GradientStop, LinearGradient, PartLayoutStyle,
        PartStyle, RadialGradient,
    };

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

    fn rgba(r: f32, g: f32, b: f32) -> ColorRef {
        ColorRef::Rgba([r, g, b, 1.0])
    }

    fn has_rect(out: &[RectInstance], color: [f32; 4], rect: [f32; 4]) -> bool {
        out.iter()
            .any(|inst| inst.color == color && inst.rect == rect)
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
    fn linear_gradient_background_emits_gradient_rect_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 180.0,
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
        assert!((fill.paint[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn radial_gradient_background_emits_gradient_rect_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::RadialGradient(RadialGradient {
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
    }

    #[test]
    fn semantic_pseudo_visuals_resolve_from_widget_state() {
        let mut dropdown = node("mode", WidgetKind::Dropdown);
        dropdown.style.open.border_color = Some(rgba(0.2, 0.4, 0.6));
        let mut state = WidgetState {
            open_dropdown: Some("mode".to_string()),
            ..Default::default()
        };

        let visual = visual_for(&dropdown, &state);
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

        let visual = visual_for(&tab, &state);
        assert_eq!(visual.background, Some(rgba(0.3, 0.5, 0.7)));
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

        assert!(has_rect(
            &out,
            [0.10, 0.20, 0.30, 1.0],
            [1.0, 1.0, 93.0, 38.0]
        ));
        assert!(has_rect(
            &out,
            [0.40, 0.50, 0.60, 1.0],
            [94.0, 1.0, 3.0, 38.0]
        ));
        assert!(has_rect(
            &out,
            [0.70, 0.80, 0.90, 1.0],
            [94.0, 20.0, 26.0, 2.0]
        ));
        assert!(has_rect(
            &out,
            [0.90, 0.10, 0.20, 1.0],
            [20.0, 10.0, 4.0, 20.0]
        ));
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

        assert!(has_rect(&out, theme.success, [0.0, 0.0, 48.0, 22.0]));
        assert!(has_rect(&out, theme.warning, [56.0, 0.0, 72.0, 22.0]));
        assert!(has_rect(
            &out,
            mix(theme.surface_alt, theme.warning, 0.22),
            [57.0, 1.0, 70.0, 20.0]
        ));
    }
}
