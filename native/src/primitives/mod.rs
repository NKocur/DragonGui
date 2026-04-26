use std::{borrow::Cow, collections::HashMap};

use bytemuck::{Pod, Zeroable};

use crate::document::{WidgetKind, WidgetNode};
use crate::events::{NavigationItem, WidgetState};
use crate::layout::{LayoutResult, Rect};
use crate::overlays::{menu_popup_rect, tooltip_target};
use crate::style::{
    number_stepper_width, VisualStyle, BORDER_WIDTH_LP, CARET_WIDTH_LP, CHECKBOX_BOX_LP,
    CHECKBOX_LEFT_PAD_LP, FOCUS_RING_LP, PANEL_ACCENT_WIDTH_LP, SLIDER_THUMB_WIDTH_LP,
    SLIDER_TRACK_HEIGHT_LP, SLIDER_TRACK_MARGIN_LP, TAB_ACTIVE_BAR_LP, TAB_GAP_LP,
    TAB_INACTIVE_BOTTOM_INSET_LP, TAB_TOP_INSET_LP,
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
    /// Corner radius in pixels; 0 = sharp corners.
    pub radius: f32,
    /// Explicit padding to keep stride at 48 bytes.
    pub _pad: [f32; 3],
}

static RECT_ATTRS: [wgpu::VertexAttribute; 3] = [
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
        format: wgpu::VertexFormat::Float32,
        offset: 32,
        shader_location: 2,
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
    RectInstance {
        rect,
        color,
        radius,
        _pad: [0.0; 3],
    }
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
    } else if state.pressed.as_deref() == Some(&node.id) {
        Cow::Owned(base.merged(&node.style.active))
    } else if state.hovered.as_deref() == Some(&node.id) {
        Cow::Owned(base.merged(&node.style.hover))
    } else if state.focused.as_deref() == Some(&node.id) {
        Cow::Owned(base.merged(&node.style.focus))
    } else {
        Cow::Borrowed(base)
    }
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
    } else if state.pressed.as_deref() == Some(&node.id) {
        darken(theme.accent, 0.15)
    } else if state.hovered.as_deref() == Some(&node.id)
        || state.focused.as_deref() == Some(&node.id)
    {
        mix(theme.surface_alt, theme.accent, 0.20)
    } else {
        theme.surface_alt
    }
}

fn control_border(node: &WidgetNode, theme: &Theme, state: &WidgetState) -> [f32; 4] {
    if state.is_disabled(&node.id) {
        mix(theme.border, theme.disabled, 0.45)
    } else if state.focused.as_deref() == Some(&node.id) {
        theme.accent
    } else if state.pressed.as_deref() == Some(&node.id) {
        darken(theme.accent, 0.08)
    } else if state.hovered.as_deref() == Some(&node.id) {
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
    out.push(inst(rect, border, radius));
    out.push(inst(
        inset_rect(rect, border_w),
        fill,
        (radius - border_w).max(0.0),
    ));
}

fn emit_focus_ring(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    rect: [f32; 4],
    radius: f32,
    out: &mut Vec<RectInstance>,
) {
    if state.focused.as_deref() == Some(&node.id) && !state.is_disabled(&node.id) {
        let ring = FOCUS_RING_LP * sf;
        out.push(inst(
            [
                rect[0] - ring,
                rect[1] - ring,
                rect[2] + ring * 2.0,
                rect[3] + ring * 2.0,
            ],
            with_alpha(theme.focus, 0.60),
            radius + ring,
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
    let accent_w = PANEL_ACCENT_WIDTH_LP * sf;

    if let Some(r) = layout.rects.get(&node.id) {
        let [x, y, w, h] = [r.x, r.y, r.w, r.h];
        let visual = visual_for(node, state);
        let border_w = visual.border_width.unwrap_or(BORDER_WIDTH_LP).max(0.0) * sf;
        let radius = visual.border_radius.unwrap_or(theme.radius).max(0.0) * sf;
        let styled_bg =
            resolve_color(&visual.background, theme).map(|c| apply_opacity(c, visual.opacity));
        let styled_border =
            resolve_color(&visual.border_color, theme).map(|c| apply_opacity(c, visual.opacity));
        let styled_accent =
            resolve_color(&visual.accent, theme).map(|c| apply_opacity(c, visual.opacity));
        match node.kind {
            WidgetKind::Panel => {
                let panel_radius = radius * 0.5;
                emit_bordered_rect(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    styled_bg.unwrap_or(theme.surface),
                    panel_radius,
                    border_w,
                );
                let accent_pad = (theme.spacing * 0.75 * sf).min(h * 0.25);
                let accent_h = (h - accent_pad * 2.0).max(border_w);
                out.push(inst(
                    [x + border_w, y + accent_pad, accent_w, accent_h],
                    styled_accent.unwrap_or(theme.accent),
                    accent_w * 0.5,
                ));
            }

            WidgetKind::Modal => {
                let root = root_rect(layout).unwrap_or(Rect { x, y, w, h });
                out.push(inst(
                    [root.x, root.y, root.w, root.h],
                    [0.0, 0.0, 0.0, 0.52],
                    0.0,
                ));
                let shadow = 6.0 * sf;
                out.push(inst(
                    [x + shadow, y + shadow, w, h],
                    [0.0, 0.0, 0.0, 0.35],
                    radius,
                ));
                emit_bordered_rect(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    styled_bg.unwrap_or(theme.surface),
                    radius,
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
                out.push(inst(
                    [x, y, w, h],
                    styled_bg.unwrap_or(theme.surface),
                    radius,
                ));
                out.push(inst(
                    [x + w - border_w, y, border_w, h],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::StatusBar => {
                out.push(inst(
                    [x, y, w, h],
                    styled_bg.unwrap_or(theme.surface),
                    radius,
                ));
                out.push(inst(
                    [x, y, w, border_w],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::MenuBar => {
                out.push(inst(
                    [x, y, w, h],
                    styled_bg.unwrap_or(theme.surface),
                    radius,
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
                let header_h = theme.control_height() * sf;
                out.push(inst(
                    [x, y, w, header_h],
                    styled_bg.unwrap_or(theme.surface),
                    0.0,
                ));
                out.push(inst(
                    [x, y + header_h - border_w, w, border_w],
                    styled_border.unwrap_or(theme.border),
                    0.0,
                ));
            }

            WidgetKind::Button | WidgetKind::Dropdown | WidgetKind::Menu => {
                emit_focus_ring(node, theme, sf, state, [x, y, w, h], radius, out);
                let menu_open =
                    node.kind == WidgetKind::Menu && state.open_menu.as_deref() == Some(&node.id);
                emit_bordered_rect(
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
                    radius,
                    border_w,
                );
            }

            WidgetKind::Tab => {
                let active = state.is_active_tab(&node.id);
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
                emit_focus_ring(node, theme, sf, state, [vx, vy, vw, vh], vr, out);
                let fill = if active {
                    styled_bg.unwrap_or_else(|| {
                        mix(
                            theme.surface_alt,
                            styled_accent.unwrap_or(theme.accent),
                            0.24,
                        )
                    })
                } else if state.hovered.as_deref() == Some(&node.id)
                    || state.focused.as_deref() == Some(&node.id)
                {
                    styled_bg.unwrap_or_else(|| {
                        mix(
                            theme.surface_alt,
                            styled_accent.unwrap_or(theme.accent),
                            0.12,
                        )
                    })
                } else if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.28))
                } else {
                    styled_bg.unwrap_or(theme.surface_alt)
                };
                out.push(inst(
                    [vx, vy, vw, vh],
                    styled_border.unwrap_or(if active {
                        styled_accent.unwrap_or(theme.accent)
                    } else {
                        theme.border
                    }),
                    vr,
                ));
                out.push(inst(
                    [
                        vx + border_w,
                        vy + border_w,
                        (vw - border_w * 2.0).max(1.0),
                        (vh - border_w).max(1.0),
                    ],
                    fill,
                    (vr - border_w).max(0.0),
                ));
                if active {
                    let bar_h = TAB_ACTIVE_BAR_LP * sf;
                    out.push(inst(
                        [
                            vx + border_w,
                            y + h - bar_h,
                            (vw - 2.0 * border_w).max(1.0),
                            bar_h,
                        ],
                        styled_accent.unwrap_or(theme.accent),
                        0.0,
                    ));
                }
            }

            WidgetKind::NavItem => {
                let active = state.is_active_nav_item(&node.id);
                emit_focus_ring(node, theme, sf, state, [x, y, w, h], radius, out);
                let fill = if active {
                    styled_bg.unwrap_or_else(|| {
                        mix(
                            theme.surface_alt,
                            styled_accent.unwrap_or(theme.accent),
                            0.20,
                        )
                    })
                } else {
                    styled_bg.unwrap_or_else(|| control_fill(node, theme, state))
                };
                out.push(inst([x, y, w, h], fill, radius));
                if active {
                    out.push(inst(
                        [x, y, accent_w, h],
                        styled_accent.unwrap_or(theme.accent),
                        accent_w * 0.5,
                    ));
                }
            }

            WidgetKind::TextInput => {
                emit_focus_ring(node, theme, sf, state, [x, y, w, h], radius, out);
                let fill = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else if state.hovered.as_deref() == Some(&node.id)
                    || state.focused.as_deref() == Some(&node.id)
                {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.70))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55))
                };
                emit_bordered_rect(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or_else(|| control_border(node, theme, state)),
                    fill,
                    radius,
                    border_w,
                );
                if state.focused.as_deref() == Some(&node.id) && !state.is_disabled(&node.id) {
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
                emit_focus_ring(node, theme, sf, state, [x, y, w, h], radius, out);
                let invalid = state.number_is_invalid(&node.id);
                let fill = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else if state.hovered.as_deref() == Some(&node.id)
                    || state.focused.as_deref() == Some(&node.id)
                {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.70))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.55))
                };
                emit_bordered_rect(
                    out,
                    [x, y, w, h],
                    if invalid {
                        theme.danger
                    } else {
                        styled_border.unwrap_or_else(|| control_border(node, theme, state))
                    },
                    fill,
                    radius,
                    border_w,
                );
                let step_w = number_stepper_width(w, sf);
                let step_x = x + w - step_w;
                let step_fill = if state.is_disabled(&node.id) {
                    mix(theme.surface_alt, theme.disabled, 0.30)
                } else if state.hovered.as_deref() == Some(&node.id)
                    || state.focused.as_deref() == Some(&node.id)
                {
                    mix(
                        theme.surface_alt,
                        styled_accent.unwrap_or(theme.accent),
                        0.16,
                    )
                } else {
                    theme.surface_alt
                };
                out.push(inst(
                    [step_x, y + border_w, border_w, h - border_w * 2.0],
                    theme.border,
                    0.0,
                ));
                out.push(inst(
                    [
                        step_x + border_w,
                        y + border_w,
                        (step_w - border_w).max(1.0),
                        (h * 0.5 - border_w).max(1.0),
                    ],
                    step_fill,
                    0.0,
                ));
                out.push(inst(
                    [
                        step_x + border_w,
                        y + h * 0.5,
                        (step_w - border_w).max(1.0),
                        (h * 0.5 - border_w).max(1.0),
                    ],
                    step_fill,
                    0.0,
                ));
                out.push(inst(
                    [step_x, y + h * 0.5, step_w, border_w],
                    theme.border,
                    0.0,
                ));
                if state.focused.as_deref() == Some(&node.id) && !state.is_disabled(&node.id) {
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
                let box_size = (CHECKBOX_BOX_LP * sf).min(h);
                let box_x = x + CHECKBOX_LEFT_PAD_LP * sf;
                let box_y = y + (h - box_size) * 0.5;
                if !state.is_disabled(&node.id)
                    && (state.hovered.as_deref() == Some(&node.id)
                        || state.pressed.as_deref() == Some(&node.id)
                        || state.focused.as_deref() == Some(&node.id))
                {
                    let row_fill = if state.pressed.as_deref() == Some(&node.id) {
                        with_alpha(darken(styled_accent.unwrap_or(theme.accent), 0.15), 0.20)
                    } else {
                        with_alpha(mix(theme.surface_alt, theme.accent, 0.20), 0.35)
                    };
                    out.push(inst([x, y, w, h], row_fill, radius));
                }
                emit_focus_ring(node, theme, sf, state, [x, y, w, h], radius, out);
                let checked = state.checked.get(&node.id).copied().unwrap_or(false);
                let disabled = state.is_disabled(&node.id);
                let fill = if checked {
                    if disabled {
                        theme.disabled
                    } else if state.pressed.as_deref() == Some(&node.id) {
                        darken(styled_accent.unwrap_or(theme.accent), 0.15)
                    } else {
                        styled_accent.unwrap_or(theme.accent)
                    }
                } else {
                    styled_bg.unwrap_or_else(|| {
                        mix(theme.surface, control_fill(node, theme, state), 0.55)
                    })
                };
                emit_bordered_rect(
                    out,
                    [box_x, box_y, box_size, box_size],
                    if checked {
                        if disabled {
                            theme.disabled
                        } else {
                            styled_border.unwrap_or(styled_accent.unwrap_or(theme.accent))
                        }
                    } else {
                        styled_border.unwrap_or_else(|| control_border(node, theme, state))
                    },
                    fill,
                    radius.min(box_size * 0.28),
                    border_w,
                );
                if checked {
                    let marker_size = (box_size * 0.42).max(3.0 * sf);
                    let marker_x = box_x + (box_size - marker_size) * 0.5;
                    let marker_y = box_y + (box_size - marker_size) * 0.5;
                    let marker_color = if disabled {
                        mix(theme.surface_alt, theme.disabled, 0.35)
                    } else {
                        theme.text
                    };
                    out.push(inst(
                        [marker_x, marker_y, marker_size, marker_size],
                        marker_color,
                        marker_size * 0.5,
                    ));
                }
            }

            WidgetKind::ProgressBar => {
                let track_fill = if state.is_disabled(&node.id) {
                    styled_bg.unwrap_or_else(|| mix(theme.surface_alt, theme.disabled, 0.24))
                } else {
                    styled_bg.unwrap_or_else(|| mix(theme.surface, theme.surface_alt, 0.60))
                };
                emit_bordered_rect(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    track_fill,
                    radius,
                    border_w,
                );
                let inset = (border_w + 2.0 * sf).max(border_w);
                let inner = inset_rect([x, y, w, h], inset);
                let t = state.slider_t(&node.id);
                let fill_w = inner[2] * t;
                if fill_w > 0.5 {
                    out.push(inst(
                        [inner[0], inner[1], fill_w, inner[3]],
                        if state.is_disabled(&node.id) {
                            theme.disabled
                        } else {
                            styled_accent.unwrap_or(theme.accent)
                        },
                        fill_w.min(inner[3]) * 0.5,
                    ));
                }
            }

            WidgetKind::Image => {
                emit_bordered_rect(
                    out,
                    [x, y, w, h],
                    styled_border.unwrap_or(theme.border),
                    styled_bg.unwrap_or(theme.surface_alt),
                    radius,
                    border_w,
                );
            }

            WidgetKind::Slider => {
                emit_focus_ring(node, theme, sf, state, [x, y, w, h], radius, out);
                let track_color = resolve_color(&visual.track_color, theme)
                    .map(|c| apply_opacity(c, visual.opacity))
                    .unwrap_or(theme.border);
                let thumb_color = resolve_color(&visual.thumb_color, theme)
                    .map(|c| apply_opacity(c, visual.opacity))
                    .unwrap_or_else(|| styled_accent.unwrap_or(theme.accent));
                let track_h = (SLIDER_TRACK_HEIGHT_LP * sf).max(border_w);
                let track_y = y + (h - track_h) * 0.5;
                let margin = SLIDER_TRACK_MARGIN_LP * sf;
                let track_w = (w - 2.0 * margin).max(0.0);
                out.push(inst(
                    [x + margin, track_y, track_w, track_h],
                    track_color,
                    track_h * 0.5,
                ));
                let t = state.slider_t(&node.id);
                let fill_w = track_w * t;
                if fill_w > 0.5 {
                    out.push(inst(
                        [x + margin, track_y, fill_w, track_h],
                        if state.is_disabled(&node.id) {
                            theme.disabled
                        } else {
                            thumb_color
                        },
                        fill_w.min(track_h) * 0.5,
                    ));
                }
                let thumb_w = SLIDER_THUMB_WIDTH_LP * sf;
                let thumb_h = thumb_w;
                let thumb_min = x + margin;
                let thumb_max = (x + w - margin - thumb_w).max(thumb_min);
                let thumb_x =
                    (x + margin + t * track_w - thumb_w * 0.5).clamp(thumb_min, thumb_max);
                let thumb_y = y + (h - thumb_h) * 0.5;
                out.push(inst(
                    [thumb_x, thumb_y, thumb_w, thumb_h],
                    styled_border.unwrap_or_else(|| control_border(node, theme, state)),
                    thumb_w * 0.5,
                ));
                out.push(inst(
                    inset_rect([thumb_x, thumb_y, thumb_w, thumb_h], border_w),
                    if state.is_disabled(&node.id) {
                        theme.disabled
                    } else {
                        thumb_color
                    },
                    (thumb_w * 0.5 - border_w).max(0.0),
                ));
            }

            WidgetKind::Scatter3D => {
                let border = styled_border.unwrap_or(theme.border);
                out.push(inst([x, y, w, border_w], border, 0.0));
                out.push(inst([x, y + h - border_w, w, border_w], border, 0.0));
                out.push(inst([x, y, border_w, h], border, 0.0));
                out.push(inst([x + w - border_w, y, border_w, h], border, 0.0));
            }

            WidgetKind::DataFrameTable => {
                emit_focus_ring(node, theme, sf, state, [x, y, w, h], radius, out);
                out.push(inst([x, y, w, h], theme.surface, radius));
                if let Some(table_state) = state.table(&node.id) {
                    let metrics = table::metrics(theme, sf);
                    let visible = table::visible(table_state, r, metrics);
                    let table_right = x + w;
                    let table_bottom = y + h;
                    let header_h = metrics.header_h.min(h);
                    out.push(inst(
                        [x, y, w, header_h],
                        mix(theme.surface_alt, theme.accent, 0.10),
                        radius,
                    ));
                    if header_h > radius {
                        out.push(inst(
                            [x, y + header_h - radius, w, radius],
                            mix(theme.surface_alt, theme.accent, 0.10),
                            0.0,
                        ));
                    }
                    if header_h < h {
                        out.push(inst([x, y + header_h, w, border_w], theme.border, 0.0));
                    }
                    let index_line_x = x + metrics.index_w;
                    if index_line_x < table_right {
                        out.push(inst([index_line_x, y, border_w, h], theme.border, 0.0));
                    }

                    for col_offset in 0..visible.col_count {
                        let Some((col_x, _)) = table::column_bounds(r, metrics, col_offset) else {
                            continue;
                        };
                        if col_x < table_right {
                            out.push(inst([col_x, y, border_w, h], theme.border, 0.0));
                        }
                    }

                    for row_offset in 0..visible.row_count {
                        let row = visible.first_row + row_offset;
                        let Some((row_y, row_bottom)) = table::row_bounds(r, metrics, row_offset)
                        else {
                            continue;
                        };
                        let row_h = row_bottom - row_y;
                        if table_state
                            .selected
                            .is_some_and(|(selected_row, _)| selected_row == row)
                        {
                            out.push(inst(
                                [x, row_y, w, row_h],
                                mix(theme.surface_alt, theme.accent, 0.22),
                                0.0,
                            ));
                        } else if row % 2 == 1 {
                            out.push(inst(
                                [x, row_y, w, row_h],
                                mix(theme.surface, theme.surface_alt, 0.36),
                                0.0,
                            ));
                        }
                        if row_bottom < table_bottom {
                            out.push(inst([x, row_bottom, w, border_w], theme.border, 0.0));
                        }
                    }

                    if let Some((_, selected_col)) = table_state.selected {
                        if selected_col >= visible.first_col
                            && selected_col < visible.first_col + visible.col_count
                        {
                            if let Some((col_x, col_right)) =
                                table::column_bounds(r, metrics, selected_col - visible.first_col)
                            {
                                out.push(inst(
                                    [col_x, y, col_right - col_x, h],
                                    [theme.accent[0], theme.accent[1], theme.accent[2], 0.12],
                                    0.0,
                                ));
                            }
                        }
                    }
                }
                out.push(inst([x, y, w, border_w], theme.border, 0.0));
                out.push(inst([x, y + h - border_w, w, border_w], theme.border, 0.0));
                out.push(inst([x, y, border_w, h], theme.border, 0.0));
                out.push(inst([x + w - border_w, y, border_w, h], theme.border, 0.0));
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
    let border_w = BORDER_WIDTH_LP * sf;
    let row_h = theme.control_height() * sf;
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
    emit_bordered_rect(
        out,
        [rect.x, rect.y, rect.w, rect.h],
        mix(theme.border, theme.accent, 0.18),
        theme.surface,
        radius,
        border_w,
    );
    for (idx, item) in items.iter().enumerate() {
        let y = rect.y + idx as f32 * row_h;
        let disabled = item.disabled || state.is_disabled(&item.id);
        let color = if disabled {
            mix(theme.surface, theme.disabled, 0.18)
        } else if state.hovered.as_deref() == Some(&item.id) {
            mix(theme.surface_alt, theme.accent, 0.24)
        } else {
            theme.surface_alt
        };
        out.push(inst(
            [
                rect.x + border_w,
                y + border_w,
                rect.w - border_w * 2.0,
                row_h - border_w,
            ],
            color,
            0.0,
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
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: &WidgetState,
    out: &mut Vec<RectInstance>,
) {
    if node.kind == WidgetKind::Dropdown && state.open_dropdown.as_deref() == Some(&node.id) {
        if let (Some(r), Some(items)) = (
            layout.rects.get(&node.id),
            state.dropdown_items.get(&node.id),
        ) {
            let row_h = theme.control_height() * sf;
            let menu_h = row_h * items.len() as f32;
            let radius = theme.radius * sf;
            let border_w = BORDER_WIDTH_LP * sf;
            let menu_rect = [r.x, r.y + r.h, r.w, menu_h];
            let shadow_offset = 3.0 * sf;
            out.push(inst(
                [
                    menu_rect[0] + shadow_offset,
                    menu_rect[1] + shadow_offset,
                    menu_rect[2],
                    menu_rect[3],
                ],
                [0.0, 0.0, 0.0, 0.30],
                radius,
            ));
            emit_bordered_rect(
                out,
                menu_rect,
                mix(theme.border, theme.accent, 0.18),
                theme.surface,
                radius,
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
                let color = if Some(idx) == hovered && idx == selected {
                    mix(theme.surface_alt, theme.accent, 0.42)
                } else if Some(idx) == hovered {
                    mix(theme.surface_alt, theme.accent, 0.24)
                } else if idx == selected {
                    mix(theme.surface_alt, theme.accent, 0.28)
                } else {
                    theme.surface_alt
                };
                out.push(inst(
                    [
                        r.x + border_w,
                        y + border_w,
                        r.w - border_w * 2.0,
                        row_h - border_w,
                    ],
                    color,
                    0.0,
                ));
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
