use std::{borrow::Cow, collections::HashMap};

use bytemuck::{Pod, Zeroable};

use crate::css_style::{
    computed_style_for_virtual_element_with_media, DgMediaEnvironment, StylesheetStore,
};
use crate::document::{WidgetKind, WidgetNode};
use crate::events::{NavigationItem, SortDirection, WidgetState};
use crate::layout::{
    is_scroll_container_node, panel_title_body_gap_lp, panel_title_line_height_lp,
    panel_title_top_padding_lp, scroll_container_max_x, scroll_container_max_y, LayoutResult, Rect,
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
    BackdropFilterStyle, BackgroundPaint, ColorRef, GradientInterpolation, NodeStyle, PartStyle,
    PositionStyle, TransformStyle, TransitionProperty, VisualStyle, BORDER_WIDTH_LP,
    CARET_WIDTH_LP, CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP, DROPDOWN_CHEVRON_WIDTH_LP,
    FOCUS_RING_LP, PANEL_ACCENT_WIDTH_LP, SLIDER_THUMB_WIDTH_LP, SLIDER_TRACK_HEIGHT_LP,
    SLIDER_TRACK_MARGIN_LP, TAB_ACTIVE_BAR_LP, TAB_GAP_LP, TAB_INACTIVE_BOTTOM_INSET_LP,
    TAB_TOP_INSET_LP,
};
use crate::table;
use crate::theme::{Color, Theme};
use crate::toast::{toast_colors, toast_rect, toast_stack_index, ToastOverlay};

const SCROLLBAR_VISIBILITY_EPSILON_PX: f32 = 2.0;
const SCROLLBAR_MIN_TRACK_LEN_PX: f32 = 44.0;
const IMPLICIT_PANEL_SCROLLBAR_MIN_SIZE_PX: f32 = 64.0;

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
    overlay_start: u32,
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
        emit_rects_inner(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            caret_positions,
            true,
            &mut self.instances,
        );
        self.overlay_start = self.instances.len() as u32;
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
        emit_modal_overlays(
            tree,
            layout,
            theme,
            scale_factor,
            state,
            caret_positions,
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

    pub fn render_base(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.rect_count == 0 {
            return;
        }
        let count = self.overlay_start.min(self.rect_count);
        if count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..6, 0..count);
    }

    pub fn render_overlays(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.rect_count == 0 || self.overlay_start >= self.rect_count {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..6, self.overlay_start..self.rect_count);
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

fn stacking_children(node: &WidgetNode) -> Vec<(usize, &WidgetNode)> {
    let mut children: Vec<_> = node.children.iter().enumerate().collect();
    children.sort_by_key(|(index, child)| (child.style.layout.z_index.unwrap_or(0), *index));
    children
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

fn resolve_color(color: &Option<crate::style::ColorRef>, theme: &Theme) -> Option<[f32; 4]> {
    color.as_ref().map(|c| c.resolve(theme))
}

#[derive(Debug, Clone)]
enum FillPaint {
    Solid([f32; 4]),
    Layers(Vec<FillPaint>),
    LinearGradient {
        colors: [[f32; 4]; GRADIENT_STOP_CAPACITY],
        stops: [f32; GRADIENT_STOP_CAPACITY],
        count: f32,
        interpolation: f32,
        angle_deg: f32,
    },
    RadialGradient {
        colors: [[f32; 4]; GRADIENT_STOP_CAPACITY],
        stops: [f32; GRADIENT_STOP_CAPACITY],
        count: f32,
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
}

const GRADIENT_STOP_CAPACITY: usize = 6;

fn apply_opacity(mut color: [f32; 4], opacity: Option<f32>) -> [f32; 4] {
    if let Some(opacity) = opacity {
        color[3] *= opacity.clamp(0.0, 1.0);
    }
    color
}

const LINE_PLOT_MAX_SEGMENTS_PER_SERIES: usize = 4096;
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
    for (series_index, series) in node.props.line_plot_series.iter().enumerate() {
        if series.points.len() < 2 {
            continue;
        }
        let color = series
            .color
            .as_ref()
            .map(|color| color.resolve(theme))
            .unwrap_or(LINE_PLOT_PALETTE[series_index % LINE_PLOT_PALETTE.len()]);
        emit_line_plot_series(
            out,
            &series.points,
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
        for point in &series.points {
            let [px, py] = *point;
            if !px.is_finite() || !py.is_finite() {
                continue;
            }
            bounds.x_min = bounds.x_min.min(px);
            bounds.x_max = bounds.x_max.max(px);
            bounds.y_min = bounds.y_min.min(py);
            bounds.y_max = bounds.y_max.max(py);
            has_point = true;
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

fn line_plot_legend_rect(node: &WidgetNode, plot: [f32; 4], sf: f32) -> Option<[f32; 4]> {
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
    let longest = labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(1) as f32;
    let label_w = (longest * 5.6 * sf).clamp(22.0 * sf, 86.0 * sf);
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
    let Some(rect) = line_plot_legend_rect(node, plot, sf) else {
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

    if let Some(legend_rect) = line_plot_legend_rect(node, plot, sf) {
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
    let mut prev: Option<[f32; 2]> = None;
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
            &mut prev,
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
            &mut prev,
        );
    }
}

fn emit_line_plot_point(
    out: &mut Vec<RectInstance>,
    point: [f32; 2],
    plot: [f32; 4],
    bounds: LinePlotBounds,
    line_width: f32,
    color: [f32; 4],
    line_style: &str,
    prev: &mut Option<[f32; 2]>,
) {
    let mapped = map_line_plot_point(point, plot, bounds);
    let Some(mapped) = mapped else {
        *prev = None;
        return;
    };
    if let Some(previous) = *prev {
        if let Some((start, end)) = clip_line_segment_to_rect(previous, mapped, plot) {
            push_styled_line_segment(out, start, end, line_width, color, line_style);
        }
    }
    *prev = Some(mapped);
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
    if len <= 0.25 {
        let radius = width * 0.5;
        out.push(inst_radii(
            [start[0] - radius, start[1] - radius, width, width],
            color,
            [radius; 4],
        ));
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
    out.push(segment);
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
    let implicit_panel_scrollbar = node.kind == WidgetKind::Panel
        && node.style.layout.overflow.is_none()
        && node.style.layout.overflow_x.is_none()
        && node.style.layout.overflow_y.is_none();
    if implicit_panel_scrollbar && rect.h < IMPLICIT_PANEL_SCROLLBAR_MIN_SIZE_PX * sf {
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
    if node.kind != WidgetKind::Panel {
        return 0.0;
    }
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
        + panel_title_body_gap_lp(node, theme))
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
                interpolation: gradient_interpolation_mode(visual.gradient_interpolation),
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

fn gradient_interpolation_mode(mode: Option<GradientInterpolation>) -> f32 {
    match mode.unwrap_or(GradientInterpolation::Srgb) {
        GradientInterpolation::Srgb => 0.0,
        GradientInterpolation::LinearSrgb => 1.0,
        GradientInterpolation::Oklab => 2.0,
    }
}

fn resolve_gradient_stops(
    stops: &[crate::style::GradientStop],
    theme: &Theme,
    opacity: Option<f32>,
) -> (
    [[f32; 4]; GRADIENT_STOP_CAPACITY],
    [f32; GRADIENT_STOP_CAPACITY],
    u32,
) {
    let resolved: Vec<([f32; 4], f32)> = normalize_gradient_stops(stops, theme, opacity);
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
            colors,
            stops,
            count,
            interpolation,
            angle_deg,
        } => out.push(inst_linear_gradient(
            rect,
            colors,
            stops,
            count,
            interpolation,
            radii,
            angle_deg,
        )),
        FillPaint::RadialGradient {
            colors,
            stops,
            count,
            interpolation,
            center,
        } => out.push(inst_radial_gradient(
            rect,
            colors,
            stops,
            count,
            interpolation,
            radii,
            center,
        )),
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
    let has_outline = visual.outline_width.is_some() || visual.outline_color.is_some();
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
    out.push(inst_outline_ring_clipped(
        outer,
        color,
        outer_radii,
        width,
        local_clip,
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
    caret_positions: &HashMap<String, [f32; 2]>,
    out: &mut Vec<RectInstance>,
) {
    emit_rects_inner(node, layout, theme, sf, state, caret_positions, false, out);
}

fn emit_rects_inner(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
    skip_open_modals: bool,
    out: &mut Vec<RectInstance>,
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
            emit_box_shadows(
                out,
                [full_rect.x, full_rect.y, full_rect.w, full_rect.h],
                radii,
                &visual,
                theme,
                sf,
                paint_clip,
            );
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
                    single_part_mark_color(node, state, theme, "indicator", theme.muted_text),
                    expanded,
                    sf,
                    layout.visible_rect(&node.id),
                );
            }

            WidgetKind::Modal => {
                let root = root_rect(layout).unwrap_or(Rect { x, y, w, h });
                let scrim_visual = part_visual_for(node, state, "scrim");
                let scrim_fallback = apply_opacity([0.0, 0.0, 0.0, 0.52], scrim_visual.opacity);
                emit_paint_rect_radii(
                    out,
                    [root.x, root.y, root.w, root.h],
                    resolve_background_paint(&scrim_visual, theme, scrim_fallback),
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
                    let title_band_h = ((panel_title_top_padding_lp(node, theme)
                        + panel_title_line_height_lp(node, theme))
                        * sf)
                        .min(inner_h);
                    let border_color = styled_border.unwrap_or(theme.border);
                    if title_band_h > 0.0 {
                        if border_w > 0.0 {
                            out.push(inst_radii([x, y, w, h], border_color, radii));
                        }
                        let base_fill =
                            resolve_color(&visual.background, theme).unwrap_or(theme.surface);
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
                                resolve_background_paint(&visual, theme, theme.surface),
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
                    let fill = resolve_background_paint(&visual, theme, theme.surface);
                    emit_underpainted_bordered_paint_rect_radii(
                        out,
                        [x, y, w, h],
                        styled_border.unwrap_or(theme.border),
                        fill,
                        radii,
                        border_w,
                    );
                }
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
                let menu_fill = visual
                    .background_paint
                    .as_ref()
                    .map(|_| resolve_background_paint(&visual, theme, theme.surface_alt))
                    .or_else(|| styled_bg.map(FillPaint::Solid))
                    .or_else(|| {
                        if state.open_menu.as_deref() == Some(node.id.as_str()) {
                            Some(FillPaint::Solid(mix(theme.surface_alt, theme.accent, 0.24)))
                        } else if state.hovered.as_deref() == Some(node.id.as_str()) {
                            Some(FillPaint::Solid(mix(theme.surface_alt, theme.accent, 0.14)))
                        } else {
                            None
                        }
                    });
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
                                resolve_part_background_paint(&glow_visual, theme, glow_color);
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
                    resolve_background_paint(&dot_visual, theme, fill_solid)
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
                    // Side-by-side layout: accent bar on the left edge, item background
                    // immediately to its right. They share no pixels so the bar looks
                    // like an integrated part of the item rather than an overlay.
                    let accent_fill = apply_opacity(
                        resolve_color(&accent_visual.background, theme)
                            .or(resolve_color(&accent_visual.foreground, theme))
                            .unwrap_or_else(|| styled_accent.unwrap_or(theme.accent)),
                        accent_visual.opacity,
                    );
                    // Bar inherits the item's outer left corners; right corners are square
                    // so it flush-joins the item background.
                    let accent_radii = visual_radii_with_fallback(
                        &accent_visual,
                        [item_radii[0], 0.0, 0.0, item_radii[3]],
                        sf,
                    );
                    if accent_border_w > 0.0 {
                        emit_bordered_rect_radii(
                            out,
                            [x, y, bar_w, h],
                            resolve_color(&accent_visual.border_color, theme)
                                .unwrap_or(accent_fill),
                            accent_fill,
                            accent_radii,
                            accent_border_w,
                        );
                    } else {
                        out.push(inst_radii([x, y, bar_w, h], accent_fill, accent_radii));
                    }
                    // Item starts right of the bar; left corners are square to match the bar.
                    let item_rect = [x + bar_w, y, (w - bar_w).max(0.0), h];
                    let item_rect_radii = [0.0, item_radii[1], item_radii[2], 0.0];
                    if item_border_w > 0.0 {
                        emit_bordered_rect_radii(
                            out,
                            item_rect,
                            resolve_color(&item_visual.border_color, theme).unwrap_or(theme.border),
                            item_fill,
                            item_rect_radii,
                            item_border_w,
                        );
                    } else {
                        out.push(inst_radii(item_rect, item_fill, item_rect_radii));
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

            WidgetKind::Histogram => emit_histogram(
                out,
                node,
                theme,
                sf,
                [x, y, w, h],
                styled_bg,
                styled_border,
                styled_accent,
                radii,
                border_w,
            ),

            WidgetKind::LinePlot => emit_line_plot(
                out,
                node,
                theme,
                sf,
                [x, y, w, h],
                styled_bg,
                styled_border,
                radii,
                border_w,
            ),

            WidgetKind::Scatter3D => {
                emit_bordered_rect_radii(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    styled_bg.unwrap_or([0.0, 0.0, 0.0, 0.0]),
                    radii,
                    border_w,
                );
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
                    let visible = table::visible(table_state, &full_rect, metrics);
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
                        let Some((col_x, _)) =
                            table::column_bounds(&full_rect, metrics, col_offset)
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

                    if let Some((sort_col, direction)) = table_state.sort {
                        if sort_col >= visible.first_col
                            && sort_col < visible.first_col + visible.col_count
                            && header_h > 0.0
                        {
                            if let Some((_, col_right)) = table::column_bounds(
                                &full_rect,
                                metrics,
                                sort_col - visible.first_col,
                            ) {
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
                }
                let border_color = styled_border.unwrap_or(grid_color);
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

    for (_, child) in stacking_children(node) {
        emit_rects_inner(
            child,
            layout,
            theme,
            sf,
            state,
            caret_positions,
            skip_open_modals,
            out,
        );
    }
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

fn emit_modal_overlays(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    caret_positions: &HashMap<String, [f32; 2]>,
    out: &mut Vec<RectInstance>,
) {
    if node.kind == WidgetKind::Modal && node.props.open.unwrap_or(false) {
        emit_rects_inner(node, layout, theme, sf, state, caret_positions, false, out);
        return;
    }
    for child in &node.children {
        emit_modal_overlays(child, layout, theme, sf, state, caret_positions, out);
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
    use crate::document::NodeProps;
    use crate::style::{
        BackdropFilterStyle, BackgroundPaint, BlobGradient, BlobGradientStop, BoxShadow, ColorRef,
        GradientStop, LinearGradient, MeshGradient, OverflowStyle, PartLayoutStyle, PartStyle,
        RadialGradient, TextStyle, VisualStyle,
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
    fn six_stop_linear_gradient_emits_extended_stop_data() {
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
                        color: ColorRef::Rgba([1.0, 0.5, 0.0, 1.0]),
                        position: Some(0.18),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([1.0, 1.0, 0.0, 1.0]),
                        position: Some(0.34),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 1.0, 0.0, 1.0]),
                        position: Some(0.52),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.0, 0.0, 1.0, 1.0]),
                        position: Some(0.76),
                    },
                    GradientStop {
                        color: ColorRef::Rgba([0.5, 0.0, 1.0, 1.0]),
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
                        position: Some(0.0),
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
        let title_inset = panel_scrollbar_title_inset(&panel, &theme, 1.0);
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
        let title_inset = panel_scrollbar_title_inset(&panel, &Theme::dark(), 1.0);

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
        let title_band_h =
            panel_title_top_padding_lp(&modal, &theme) + panel_title_line_height_lp(&modal, &theme);
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
        emit_rects_inner(&root, &layout, &theme, 1.0, &state, &carets, true, &mut out);
        emit_modal_overlays(&root, &layout, &theme, 1.0, &state, &carets, &mut out);

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
