use std::{borrow::Cow, collections::HashMap};

use bytemuck::{Pod, Zeroable};

use crate::css_style::{
    computed_style_for_virtual_element_with_media, DgMediaEnvironment, StylesheetStore,
};
use crate::document::{WidgetKind, WidgetNode};
use crate::events::{NavigationItem, SortDirection, WidgetState};
use crate::layout::{
    panel_title_gap_lp, panel_title_line_height_lp, panel_title_top_padding_lp,
    scroll_container_max_x, scroll_container_max_y, LayoutResult, Rect,
};
use crate::overlays::{menu_popup_rect, rich_tooltip_target, tooltip_target};
use crate::style::{
    badge_height_for_style, badge_width_for_text, base_part_style, checked_part_style_for_state,
    collapsed_part_style_for_state, collapsible_header_height_for_style,
    expanded_part_style_for_state,
    merged_part_visual_for_state as style_merged_part_visual_for_state,
    number_stepper_width_for_style, open_part_style_for_state,
    part_style_active_for_state as style_part_style_active_for_state,
    part_visual_for_state as style_part_visual_for_state, selected_part_style_for_state,
    state_part_style_for_state, tabs_header_height_for_style, uniform_layout_padding,
    BackdropFilterStyle, BackgroundPaint, ColorRef, NodeStyle, PartStyle, PositionStyle,
    TransformStyle, TransitionProperty, VisualStyle, BORDER_WIDTH_LP, CARET_WIDTH_LP,
    CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP, DROPDOWN_CHEVRON_WIDTH_LP, FOCUS_RING_LP,
    PANEL_ACCENT_WIDTH_LP, SLIDER_THUMB_WIDTH_LP, SLIDER_TRACK_HEIGHT_LP, SLIDER_TRACK_MARGIN_LP,
    TAB_ACTIVE_BAR_LP, TAB_GAP_LP, TAB_INACTIVE_BOTTOM_INSET_LP, TAB_TOP_INSET_LP,
};
use crate::table;
use crate::theme::{Color, Theme};
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
}

static RECT_ATTRS: [wgpu::VertexAttribute; 12] = [
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
        media: DgMediaEnvironment,
    ) {
        let window_w = media.width * scale_factor;
        let window_h = media.height * scale_factor;
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
            media,
            &mut self.instances,
        );
        emit_toast_overlays(
            toasts,
            theme,
            scale_factor,
            stylesheets,
            media,
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
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: color,
        color4: color,
        gradient_stops: [0.0, 1.0, 1.0, 1.0],
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
        transform: [0.0, 0.0, 1.0, 1.0],
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: color,
        color4: color,
        gradient_stops: [0.0, 1.0, 1.0, 1.0],
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
    }
}

fn inst_linear_gradient(
    rect: [f32; 4],
    colors: [[f32; 4]; 4],
    stops: [f32; 4],
    count: f32,
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
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: colors[2],
        color4: colors[3],
        gradient_stops: stops,
    }
}

fn inst_radial_gradient(
    rect: [f32; 4],
    colors: [[f32; 4]; 4],
    stops: [f32; 4],
    count: f32,
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
        transform2: [0.0, 0.0, 0.0, 0.0],
        color3: colors[2],
        color4: colors[3],
        gradient_stops: stops,
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

fn apply_transform_to_instances(
    instances: &mut [RectInstance],
    transform: Option<TransformStyle>,
    sf: f32,
) {
    let Some(transform) = transform.filter(|transform| !transform.is_identity()) else {
        return;
    };
    let encoded = [
        transform.translate_x * sf,
        transform.translate_y * sf,
        transform.scale_x,
        transform.scale_y,
    ];
    let rotation = transform.rotate_deg.to_radians();
    for instance in instances {
        instance.transform = encoded;
        instance.transform2[0] = rotation;
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
    if filter.blur <= 0.0 {
        return;
    }
    let alpha = (filter.blur / 180.0).clamp(0.025, 0.095);
    out.push(inst_radii(rect, [1.0, 1.0, 1.0, alpha], radii));
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

fn stacking_children(node: &WidgetNode) -> Vec<(usize, &WidgetNode)> {
    let mut children: Vec<_> = node.children.iter().enumerate().collect();
    children.sort_by_key(|(index, child)| (child.style.layout.z_index.unwrap_or(0), *index));
    children
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

fn visual_for<'a>(
    node: &'a WidgetNode,
    state: &WidgetState,
    theme: &Theme,
) -> Cow<'a, VisualStyle> {
    let base = &node.style.visual;
    let mut visual = base.clone();
    let mut changed = false;
    merge_checked_visual_state(&mut visual, node, state, &mut changed);
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
    if state.pressed.as_deref() == Some(node.id.as_str()) {
        visual = visual.merged(&node.style.active);
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

fn resolve_color(color: &Option<crate::style::ColorRef>, theme: &Theme) -> Option<[f32; 4]> {
    color.as_ref().map(|c| c.resolve(theme))
}

#[derive(Debug, Clone)]
enum FillPaint {
    Solid([f32; 4]),
    Layers(Vec<FillPaint>),
    LinearGradient {
        colors: [[f32; 4]; 4],
        stops: [f32; 4],
        count: f32,
        angle_deg: f32,
    },
    RadialGradient {
        colors: [[f32; 4]; 4],
        stops: [f32; 4],
        count: f32,
        center: [f32; 2],
    },
}

fn apply_opacity(mut color: [f32; 4], opacity: Option<f32>) -> [f32; 4] {
    if let Some(opacity) = opacity {
        color[3] *= opacity.clamp(0.0, 1.0);
    }
    color
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
    let has_horizontal = max_scroll_x > 0.0;
    let has_vertical = max_scroll_y > 0.0;
    if !has_horizontal && !has_vertical {
        return None;
    }
    if rect.w <= 0.0 || rect.h <= 0.0 {
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
    let title_inset = panel_scrollbar_title_inset(node, theme, sf).min(rect.h.max(0.0));
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
        let vertical_pad = part_padding.unwrap_or(default_vertical_pad);
        let right_pad = (border_w + gap).max(right_radius * 0.45);
        let horizontal_reserve = if has_horizontal {
            gutter_thickness + gap
        } else {
            0.0
        };
        let gutter_x = rect.x + rect.w - right_pad - gutter_thickness;
        let track_x = gutter_x + (gutter_thickness - track_thickness) * 0.5;
        let track_y = rect.y + vertical_pad;
        let track_bottom = rect.y + rect.h - vertical_pad - horizontal_reserve;
        let track_h = (track_bottom - track_y).max(1.0);
        if gutter_x >= rect.x && track_h > 1.0 {
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
        let horizontal_pad = part_padding.unwrap_or(default_horizontal_pad);
        let bottom_pad = (border_w + gap).max(bottom_radius * 0.45);
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
        if gutter_y >= rect.y && track_w > 1.0 {
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
    let track_fallback = with_alpha(mix(theme.surface, theme.muted_text, 0.25), 0.22);
    let thumb_fallback = with_alpha(mix(theme.surface_alt, theme.muted_text, 0.45), 0.58);

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

fn panel_scrollbar_title_inset(node: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
    if !node
        .props
        .text
        .as_deref()
        .is_some_and(|text| !text.is_empty())
    {
        return 0.0;
    }
    (panel_title_top_padding_lp(node, theme)
        + panel_title_line_height_lp(node, theme)
        + panel_title_gap_lp(node, theme))
        * sf
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
    let paint = resolve_part_background_paint(visual, theme, fallback_color);
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
) -> FillPaint {
    if visual.background_paint.is_some() || visual.background.is_some() {
        resolve_background_paint(visual, theme, fallback)
    } else {
        FillPaint::Solid(apply_opacity(fallback, visual.opacity))
    }
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
        Some(BackgroundPaint::Layers(layers)) if !layers.is_empty() => FillPaint::Layers(
            layers
                .iter()
                .map(|paint| resolve_background_paint_layer(paint, visual, theme, fallback))
                .collect(),
        ),
        Some(paint) => resolve_background_paint_layer(paint, visual, theme, fallback),
        None => FillPaint::Solid(
            resolve_color(&visual.background, theme)
                .map(|color| apply_opacity(color, visual.opacity))
                .unwrap_or(fallback),
        ),
    }
}

fn resolve_background_paint_layer(
    paint: &BackgroundPaint,
    visual: &VisualStyle,
    theme: &Theme,
    fallback: [f32; 4],
) -> FillPaint {
    match paint {
        BackgroundPaint::Color(color) => {
            FillPaint::Solid(apply_opacity(color.resolve(theme), visual.opacity))
        }
        BackgroundPaint::LinearGradient(gradient) if gradient.stops.len() >= 2 => {
            let (colors, stops, count) =
                resolve_gradient_stops(&gradient.stops, theme, visual.opacity);
            FillPaint::LinearGradient {
                colors,
                stops,
                count: signed_gradient_stop_count(count, gradient.repeating),
                angle_deg: gradient.angle_deg,
            }
        }
        BackgroundPaint::RadialGradient(gradient) if gradient.stops.len() >= 2 => {
            let (colors, stops, count) =
                resolve_gradient_stops(&gradient.stops, theme, visual.opacity);
            FillPaint::RadialGradient {
                colors,
                stops,
                count: signed_gradient_stop_count(count, gradient.repeating),
                center: gradient.center,
            }
        }
        BackgroundPaint::Layers(layers) if !layers.is_empty() => FillPaint::Layers(
            layers
                .iter()
                .map(|paint| resolve_background_paint_layer(paint, visual, theme, fallback))
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

fn resolve_gradient_stops(
    stops: &[crate::style::GradientStop],
    theme: &Theme,
    opacity: Option<f32>,
) -> ([[f32; 4]; 4], [f32; 4], u32) {
    let resolved: Vec<([f32; 4], f32)> = normalize_gradient_stops(stops, theme, opacity);
    if resolved.len() <= 4 {
        let mut colors = [[0.0, 0.0, 0.0, 0.0]; 4];
        let mut positions = [0.0, 1.0, 1.0, 1.0];
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

    let sample_positions = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0];
    let mut colors = [[0.0, 0.0, 0.0, 0.0]; 4];
    for (index, position) in sample_positions.iter().enumerate() {
        colors[index] = gradient_color_at(&resolved, *position);
    }
    (colors, sample_positions, 4)
}

fn normalize_gradient_stops(
    stops: &[crate::style::GradientStop],
    theme: &Theme,
    opacity: Option<f32>,
) -> Vec<([f32; 4], f32)> {
    let len = stops.len();
    let mut positions: Vec<Option<f32>> = stops
        .iter()
        .map(|stop| stop.position.map(|position| position.clamp(0.0, 1.0)))
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
            (apply_opacity(stop.color.resolve(theme), opacity), position)
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
            return mix(left_color, right_color, (position - left_pos) / span);
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
            colors,
            stops,
            count,
            angle_deg,
        } => out.push(inst_linear_gradient(
            rect, colors, stops, count, radii, angle_deg,
        )),
        FillPaint::RadialGradient {
            colors,
            stops,
            count,
            center,
        } => out.push(inst_radial_gradient(
            rect, colors, stops, count, radii, center,
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
        let own_primitive_start = out.len();
        let [x, y, w, h] = [r.x, r.y, r.w, r.h];
        let visual = visual_for(node, state, theme);
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
                    single_part_mark_color(node, state, theme, "indicator", theme.muted_text),
                    expanded,
                    sf,
                    layout.visible_rect(&node.id),
                );
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

            WidgetKind::Menu => {
                let menu_radius_lp = visual.border_radius.unwrap_or(4.0).max(0.0);
                let menu_radii = visual_radii(&visual, menu_radius_lp, sf);
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], menu_radii, out);
                let menu_open = state.open_menu.as_deref() == Some(node.id.as_str());
                let menu_fill = if visual.background_paint.is_some() {
                    Some(resolve_background_paint(&visual, theme, theme.surface_alt))
                } else {
                    styled_bg
                        .or_else(|| {
                            if state.is_disabled(&node.id) {
                                None
                            } else if menu_open {
                                Some(mix(
                                    theme.surface_alt,
                                    styled_accent.unwrap_or(theme.accent),
                                    0.24,
                                ))
                            } else if state.pressed.as_deref() == Some(node.id.as_str()) {
                                Some(mix(
                                    theme.surface_alt,
                                    styled_accent.unwrap_or(theme.accent),
                                    0.20,
                                ))
                            } else if state.hovered.as_deref() == Some(node.id.as_str())
                                || state.focused.as_deref() == Some(node.id.as_str())
                            {
                                Some(mix(
                                    theme.surface_alt,
                                    styled_accent.unwrap_or(theme.accent),
                                    0.14,
                                ))
                            } else {
                                None
                            }
                        })
                        .map(FillPaint::Solid)
                };
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

            WidgetKind::Button | WidgetKind::Dropdown => {
                emit_focus_ring_radii(node, theme, sf, state, [x, y, w, h], radii, out);
                let fill = if visual.background_paint.is_some() {
                    resolve_background_paint(&visual, theme, control_fill(node, theme, state))
                } else {
                    FillPaint::Solid(styled_bg.unwrap_or_else(|| control_fill(node, theme, state)))
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
                } else if node.kind == WidgetKind::Dropdown {
                    let chevron_w = dropdown_chevron_width_for_style(node, sf);
                    let chevron_rect = [x + w - theme.spacing * sf - chevron_w, y, chevron_w, h];
                    emit_dropdown_chevron(
                        out,
                        chevron_rect,
                        single_part_mark_color(node, state, theme, "chevron", theme.muted_text),
                        state.open_dropdown.as_deref() == Some(node.id.as_str()),
                        sf,
                    );
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
                let tab_radii = visual_radii_with_fallback(&tab_visual, [vr, vr, 0.0, 0.0], sf);
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
                    let accent_rect = [vx, vy + vh - bar_h, vw, bar_h];
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
                    let accent_border_w = accent_visual
                        .border_width
                        .map(|width| (width.max(0.0) * sf).max(0.0))
                        .unwrap_or(0.0);
                    let inset = item_border_w.max(accent_border_w);
                    let accent_rect = [
                        x + inset,
                        y + inset,
                        bar_w.min((w - inset * 2.0).max(1.0)),
                        (h - inset * 2.0).max(1.0),
                    ];
                    let accent_fill = apply_opacity(
                        resolve_color(&accent_visual.background, theme)
                            .or(resolve_color(&accent_visual.foreground, theme))
                            .unwrap_or_else(|| styled_accent.unwrap_or(theme.accent)),
                        accent_visual.opacity,
                    );
                    let accent_radii = visual_radii_with_fallback(
                        &accent_visual,
                        [
                            (item_radii[0] - inset).max(0.0),
                            0.0,
                            0.0,
                            (item_radii[3] - inset).max(0.0),
                        ],
                        sf,
                    );
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
                    .unwrap_or(theme.border);
                let stepper_divider_color =
                    resolve_color(&stepper_divider_visual.background, theme)
                        .or_else(|| resolve_color(&stepper_divider_visual.border_color, theme))
                        .map(|color| apply_opacity(color, stepper_divider_visual.opacity))
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

                    if let Some((sort_col, direction)) = table_state.sort {
                        if sort_col >= visible.first_col
                            && sort_col < visible.first_col + visible.col_count
                            && header_h > 0.0
                        {
                            if let Some((_, col_right)) =
                                table::column_bounds(&r, metrics, sort_col - visible.first_col)
                            {
                                let indicator_w = DROPDOWN_CHEVRON_WIDTH_LP * sf;
                                let inset = (theme.spacing * 0.5 * sf).max(2.0 * sf);
                                let marker_right = col_right.min(table_right) - inset;
                                let marker_x = marker_right - indicator_w;
                                if marker_x > x + metrics.index_w && marker_right > marker_x {
                                    let color = single_part_mark_color(
                                        node,
                                        state,
                                        theme,
                                        "header",
                                        theme.muted_text,
                                    );
                                    let clip = Rect {
                                        x,
                                        y,
                                        w,
                                        h: header_h,
                                    };
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
        if let Some(filter) = visual
            .backdrop_filter
            .filter(|_| widget_supports_backdrop_filter(node.kind))
        {
            emit_backdrop_filter_tint(out, [x, y, w, h], radii, filter);
        }
        if widget_supports_box_shadow(node.kind) || node.kind == WidgetKind::Modal {
            emit_inset_box_shadows(out, [x, y, w, h], radii, &visual, theme, sf);
        }
        apply_transform_to_instances(
            &mut out[own_primitive_start..],
            paint_transform_for_node(node, visual.transform),
            sf,
        );
        apply_background_noise_to_instances(
            &mut out[own_primitive_start..],
            visual
                .background_noise
                .or_else(|| backdrop_filter_noise(&visual)),
        );
    }

    for (_, child) in stacking_children(node) {
        emit_rects(child, layout, theme, sf, state, caret_positions, out);
    }
    if node.kind == WidgetKind::Panel {
        if let Some(r) = layout.visible_rect(&node.id) {
            emit_panel_scrollbar(node, layout, state, theme, sf, [r.x, r.y, r.w, r.h], out);
        }
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
    let Some((_node, rect)) = tooltip_target(tree, layout, theme, state, sf) else {
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
    let border_w = BORDER_WIDTH_LP * sf;
    let visual = visual_for(node, state, theme);
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
        PartStyle, RadialGradient, TextStyle,
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
    fn multi_stop_linear_gradient_emits_stop_data() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 90.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 0.0, 0.0, 1.0]),
                        position: Some(0.0),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 1.0, 0.0, 1.0]),
                        position: Some(0.25),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 1.0, 1.0]),
                        position: Some(1.0),
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
    fn repeating_linear_gradient_marks_negative_stop_count() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint =
            Some(BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 90.0,
                repeating: true,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 1.0, 0.18]),
                        position: Some(0.0),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 1.0, 0.18]),
                        position: Some(0.08),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]),
                        position: Some(0.08),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]),
                        position: Some(0.16),
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
    fn layered_gradient_background_emits_back_to_front_instances() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.background_paint = Some(BackgroundPaint::Layers(vec![
            BackgroundPaint::RadialGradient(RadialGradient {
                repeating: false,
                center: [0.2, 0.25],
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 1.0, 0.18]),
                        position: Some(0.0),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]),
                        position: Some(0.65),
                    },
                ],
            }),
            BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: 135.0,
                repeating: false,
                stops: vec![
                    GradientStop {
                        color: ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]),
                        position: Some(0.0),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.1, 1.0]),
                        position: Some(1.0),
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
                        position: Some(0.0),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 0.1, 1.0]),
                        position: Some(1.0),
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
        assert!(
            top_gap >= 11.0,
            "scrollbar track should leave enough vertical breathing room: {:?}",
            track.rect
        );
        assert!(
            (top_gap - bottom_gap).abs() < 0.01,
            "scrollbar track should be vertically centered on the panel surface: top_gap={top_gap} bottom_gap={bottom_gap}"
        );
        assert!(thumb.rect[1] >= track.rect[1]);
        assert!(thumb.rect[1] + thumb.rect[3] <= track.rect[1] + track.rect[3]);
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

        assert_eq!(track.rect, [84.0, 14.0, 6.0, 92.0]);
        assert_eq!(thumb.rect, [83.0, 14.0, 8.0, 46.0]);
        assert_eq!(track.radii, [99.0; 4]);
        assert_eq!(thumb.radii, [99.0; 4]);
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
                sort: Some((1, SortDirection::Asc)),
                row_order: None,
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
        assert!(mark.rect[1] > 40.0);
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
            .find(|inst| inst.rect == [4.0, 4.0, 112.0, 32.0])
            .expect("active tab body should be emitted");
        assert!(tab_surface.radii[0] > 0.0);
        assert!(tab_surface.radii[1] > 0.0);
        assert_eq!(tab_surface.radii[2], 0.0);
        assert_eq!(tab_surface.radii[3], 0.0);
    }

    #[test]
    fn active_nav_item_accent_uses_item_left_radii() {
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
        assert_eq!(accent.rect, [0.0, 0.0, 5.0, 36.0]);
        assert_eq!(accent.radii, [8.0, 0.0, 0.0, 8.0]);
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

        let mut open_state = WidgetState {
            open_menu: Some("file-menu".to_string()),
            ..Default::default()
        };
        emit_rects(
            &menu,
            &layout,
            &theme,
            1.0,
            &open_state,
            &HashMap::new(),
            &mut out,
        );

        let open_fill = mix(theme.surface_alt, theme.accent, 0.24);
        let active = out
            .iter()
            .find(|inst| inst.color == open_fill)
            .expect("open menu should emit a subtle menu-bar highlight");
        assert_eq!(active.rect, [0.0, 0.0, 64.0, 30.0]);
        assert_eq!(active.radii, [4.0; 4]);
        assert!(!out.iter().any(|inst| inst.color == theme.border));

        open_state.open_menu = None;
        open_state.hovered = Some("file-menu".to_string());
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
        assert!(out
            .iter()
            .any(|inst| inst.color == mix(theme.surface_alt, theme.accent, 0.14)));
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

        assert!(has_rect(
            &out,
            [0.10, 0.20, 0.30, 1.0],
            [27.0, 1.0, 66.0, 38.0]
        ));
        assert!(has_rect(
            &out,
            [0.40, 0.50, 0.60, 1.0],
            [24.5, 1.0, 3.0, 38.0]
        ));
        assert!(has_rect(
            &out,
            [0.70, 0.80, 0.90, 1.0],
            [92.5, 1.0, 3.0, 38.0]
        ));
        assert!(has_rect(
            &out,
            [0.90, 0.10, 0.20, 1.0],
            [46.0, 10.0, 4.0, 20.0]
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
