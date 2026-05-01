pub mod camera;
pub mod colormap;

use bytemuck::{Pod, Zeroable};
use camera::Camera;

// ---------------------------------------------------------------------------
// GPU vertex layout
// ---------------------------------------------------------------------------

/// One point rendered as a screen-space billboard quad (6 vertices, instanced).
/// Matches @location attributes in points.wgsl.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PointInstance {
    pub position: [f32; 3], // @location(0)
    pub size: f32,          // @location(1)  pixels
    pub color: [f32; 3],    // @location(2)
    pub alpha: f32,         // @location(3)
}

static POINT_ATTRS: [wgpu::VertexAttribute; 4] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 12,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 16,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 28,
        shader_location: 3,
    },
];

fn point_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<PointInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &POINT_ATTRS,
    }
}

// ---------------------------------------------------------------------------
// Uniform block — matches Uniforms struct in points.wgsl
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    screen_size: [f32; 2],
    style: u32,
    point_size: f32,
    clip_radii: [f32; 4],
}

fn point_size_override_value(point_size: Option<f32>) -> f32 {
    point_size.map(|size| size.max(0.0)).unwrap_or(-1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScatterLayoutRect {
    offset: [f32; 2],
    width: u32,
    height: u32,
    scissor_offset: [u32; 2],
    scissor_size: [u32; 2],
}

fn scatter_layout_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    visible_clip: Option<[f32; 4]>,
) -> ScatterLayoutRect {
    let width = w.max(0.0) as u32;
    let height = h.max(0.0) as u32;
    let [clip_x, clip_y, clip_w, clip_h] = visible_clip.unwrap_or([x, y, w, h]);
    let left = clip_x.max(x);
    let top = clip_y.max(y);
    let right = (clip_x + clip_w).min(x + w).max(left);
    let bottom = (clip_y + clip_h).min(y + h).max(top);
    let scissor_x = left.floor().max(0.0) as u32;
    let scissor_y = top.floor().max(0.0) as u32;
    let scissor_right = right.ceil().max(scissor_x as f32) as u32;
    let scissor_bottom = bottom.ceil().max(scissor_y as f32) as u32;
    ScatterLayoutRect {
        offset: [x, y],
        width,
        height,
        scissor_offset: [scissor_x, scissor_y],
        scissor_size: [
            scissor_right.saturating_sub(scissor_x),
            scissor_bottom.saturating_sub(scissor_y),
        ],
    }
}

// ---------------------------------------------------------------------------
// ScatterWidget
// ---------------------------------------------------------------------------

pub struct ScatterWidget {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_cap: u64,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pub point_count: u32,
    pub camera: Camera,
    /// Viewport offset within the window (pixels, top-left origin).
    pub offset: [f32; 2],
    pub width: u32,
    pub height: u32,
    scissor_offset: [u32; 2],
    scissor_size: [u32; 2],
    /// Saved for camera reset (R / Home).
    fit_center: glam::Vec3,
    fit_radius: f32,
    point_size_override: f32,
    clip_radii: [f32; 4],
}

impl ScatterWidget {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-points"),
            source: wgpu::ShaderSource::Wgsl(include_str!("points.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scatter-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scatter-pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scatter"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[point_instance_layout()],
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scatter-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scatter-bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let fit_center = glam::Vec3::ZERO;
        let fit_radius = 5.0_f32;
        let aspect = width as f32 / height.max(1) as f32;
        let camera = Camera::fit(fit_center, fit_radius, aspect);

        Self {
            pipeline,
            vertex_buffer: None,
            vertex_cap: 0,
            uniform_buffer,
            bind_group,
            point_count: 0,
            camera,
            offset: [0.0, 0.0],
            width,
            height,
            scissor_offset: [0, 0],
            scissor_size: [width, height],
            fit_center,
            fit_radius,
            point_size_override: -1.0,
            clip_radii: [0.0; 4],
        }
    }

    /// Upload point data to GPU.  Reallocates the vertex buffer if needed.
    pub fn set_points(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        points: &[PointInstance],
    ) {
        let size = (points.len() * std::mem::size_of::<PointInstance>()) as u64;
        if size == 0 {
            self.point_count = 0;
            return;
        }
        if self.vertex_buffer.is_none() || size > self.vertex_cap {
            // Over-allocate by 2× so incremental updates don't thrash.
            let cap = (size * 2).max(4 * 1024 * 1024); // min 4 MiB
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_cap = cap;
        }
        queue.write_buffer(
            self.vertex_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(points),
        );
        self.point_count = points.len() as u32;
    }

    /// Write current camera state into the uniform buffer.
    pub fn update_camera(&self, queue: &wgpu::Queue) {
        let vp = self.camera.view_proj();
        let uniforms = Uniforms {
            view_proj: vp.to_cols_array_2d(),
            screen_size: [self.width as f32, self.height as f32],
            style: 0, // circle
            point_size: self.point_size_override,
            clip_radii: self.clip_radii,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn set_point_size_override(&mut self, point_size: Option<f32>, queue: &wgpu::Queue) {
        self.point_size_override = point_size_override_value(point_size);
        self.update_camera(queue);
    }

    fn effective_point_size(&self, base_size: f32) -> f32 {
        if self.point_size_override >= 0.0 {
            self.point_size_override
        } else {
            base_size
        }
    }

    /// Place the scatter inside a sub-region of the window.
    ///
    /// Updates the stored offset, dimensions, camera aspect ratio, and
    /// uniform buffer.  Call this after every layout recomputation.
    pub fn set_layout_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        visible_clip: Option<[f32; 4]>,
        clip_radii: [f32; 4],
        queue: &wgpu::Queue,
    ) {
        let rect = scatter_layout_rect(x, y, w, h, visible_clip);
        self.offset = rect.offset;
        self.width = rect.width;
        self.height = rect.height;
        self.scissor_offset = rect.scissor_offset;
        self.scissor_size = rect.scissor_size;
        self.clip_radii = clamp_clip_radii(clip_radii, w, h);
        self.camera.aspect = w / h.max(1.0);
        self.update_camera(queue);
    }

    /// Restore the camera to its initial fit position (R / Home key).
    pub fn reset_camera(&mut self, queue: &wgpu::Queue) {
        let aspect = self.width as f32 / self.height.max(1) as f32;
        self.camera = Camera::fit(self.fit_center, self.fit_radius, aspect);
        self.update_camera(queue);
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        let left = self.offset[0];
        let top = self.offset[1];
        let right = left + self.width as f32;
        let bottom = top + self.height as f32;
        let scissor_left = self.scissor_offset[0] as f32;
        let scissor_top = self.scissor_offset[1] as f32;
        let scissor_right = scissor_left + self.scissor_size[0] as f32;
        let scissor_bottom = scissor_top + self.scissor_size[1] as f32;
        if x < scissor_left || x >= scissor_right || y < scissor_top || y >= scissor_bottom {
            return false;
        }
        if x < left || x >= right || y < top || y >= bottom {
            return false;
        }
        rounded_clip_contains(
            x - left,
            y - top,
            self.width as f32,
            self.height as f32,
            self.clip_radii,
        )
    }

    pub fn pick_point(
        &self,
        points: &[PointInstance],
        x: f32,
        y: f32,
        radius_px: f32,
    ) -> Option<(usize, PointInstance)> {
        if !self.contains_point(x, y) || self.width == 0 || self.height == 0 {
            return None;
        }
        let local_x = x - self.offset[0];
        let local_y = y - self.offset[1];
        let view_proj = self.camera.view_proj();
        let mut best: Option<(usize, PointInstance, f32, f32)> = None;
        for (idx, point) in points.iter().copied().enumerate() {
            let clip = view_proj
                * glam::Vec4::new(point.position[0], point.position[1], point.position[2], 1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 || ndc.z < 0.0 || ndc.z > 1.0 {
                continue;
            }
            let screen_x = (ndc.x * 0.5 + 0.5) * self.width as f32;
            let screen_y = (0.5 - ndc.y * 0.5) * self.height as f32;
            let dx = screen_x - local_x;
            let dy = screen_y - local_y;
            let threshold = radius_px.max(self.effective_point_size(point.size) * 0.75);
            let dist2 = dx * dx + dy * dy;
            if dist2 > threshold * threshold {
                continue;
            }
            match best {
                Some((_, _, best_dist2, best_depth))
                    if best_dist2 < dist2
                        || ((best_dist2 - dist2).abs() <= f32::EPSILON && best_depth <= ndc.z) => {}
                _ => best = Some((idx, point, dist2, ndc.z)),
            }
        }
        best.map(|(idx, point, _, _)| (idx, point))
    }

    /// Record draw commands into an active render pass.
    ///
    /// Applies a viewport and scissor rect so the scatter only draws within
    /// its assigned region.
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.point_count == 0
            || self.width == 0
            || self.height == 0
            || self.scissor_size[0] == 0
            || self.scissor_size[1] == 0
        {
            return;
        }
        if let Some(vb) = &self.vertex_buffer {
            let w = self.width as f32;
            let h = self.height as f32;
            pass.set_viewport(self.offset[0], self.offset[1], w, h, 0.0, 1.0);
            pass.set_scissor_rect(
                self.scissor_offset[0],
                self.scissor_offset[1],
                self.scissor_size[0],
                self.scissor_size[1],
            );
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.draw(0..6, 0..self.point_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_size_override_uses_negative_sentinel_for_default() {
        assert_eq!(point_size_override_value(None), -1.0);
        assert_eq!(point_size_override_value(Some(6.0)), 6.0);
        assert_eq!(point_size_override_value(Some(-2.0)), 0.0);
    }

    #[test]
    fn clipped_scatter_keeps_full_viewport_and_visible_scissor() {
        let rect = scatter_layout_rect(20.0, 40.0, 300.0, 180.0, Some([20.0, 96.0, 300.0, 72.0]));

        assert_eq!(rect.offset, [20.0, 40.0]);
        assert_eq!(rect.width, 300);
        assert_eq!(rect.height, 180);
        assert_eq!(rect.scissor_offset, [20, 96]);
        assert_eq!(rect.scissor_size, [300, 72]);
    }

    #[test]
    fn scatter_uniform_layout_stays_wgpu_aligned() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 96);
    }

    #[test]
    fn rounded_clip_contains_respects_corner_radii() {
        assert!(rounded_clip_contains(10.0, 10.0, 100.0, 60.0, [0.0; 4]));
        assert!(!rounded_clip_contains(
            2.0,
            2.0,
            100.0,
            60.0,
            [20.0, 0.0, 0.0, 0.0]
        ));
        assert!(rounded_clip_contains(
            18.0,
            18.0,
            100.0,
            60.0,
            [20.0, 0.0, 0.0, 0.0]
        ));
    }
}

fn clamp_clip_radii(radii: [f32; 4], width: f32, height: f32) -> [f32; 4] {
    let limit = width.min(height).max(0.0) * 0.5;
    radii.map(|radius| radius.max(0.0).min(limit))
}

fn rounded_clip_contains(
    local_x: f32,
    local_y: f32,
    width: f32,
    height: f32,
    radii: [f32; 4],
) -> bool {
    if local_x < 0.0 || local_y < 0.0 || local_x >= width || local_y >= height {
        return false;
    }
    let max_radius = radii.iter().copied().fold(0.0_f32, f32::max);
    if max_radius <= f32::EPSILON {
        return true;
    }
    let cx = local_x - width * 0.5;
    let cy = local_y - height * 0.5;
    let radius = if cy < 0.0 {
        if cx < 0.0 {
            radii[0]
        } else {
            radii[1]
        }
    } else if cx < 0.0 {
        radii[3]
    } else {
        radii[2]
    };
    if radius <= f32::EPSILON {
        return true;
    }
    let half_w = width * 0.5;
    let half_h = height * 0.5;
    let qx = cx.abs() - (half_w - radius);
    let qy = cy.abs() - (half_h - radius);
    let outside_x = qx.max(0.0);
    let outside_y = qy.max(0.0);
    let outside = (outside_x * outside_x + outside_y * outside_y).sqrt();
    let inside = qx.max(qy).min(0.0);
    let dist = outside + inside - radius;
    dist <= 0.75
}
