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
    _pad: f32,
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
    /// Saved for camera reset (R / Home).
    fit_center: glam::Vec3,
    fit_radius: f32,
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
            fit_center,
            fit_radius,
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
            _pad: 0.0,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Place the scatter inside a sub-region of the window.
    ///
    /// Updates the stored offset, dimensions, camera aspect ratio, and
    /// uniform buffer.  Call this after every layout recomputation.
    pub fn set_layout_rect(&mut self, x: f32, y: f32, w: f32, h: f32, queue: &wgpu::Queue) {
        self.offset = [x, y];
        self.width = w as u32;
        self.height = h as u32;
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
        x >= left && x < right && y >= top && y < bottom
    }

    /// Record draw commands into an active render pass.
    ///
    /// Applies a viewport and scissor rect so the scatter only draws within
    /// its assigned region.
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.point_count == 0 || self.width == 0 || self.height == 0 {
            return;
        }
        if let Some(vb) = &self.vertex_buffer {
            let w = self.width as f32;
            let h = self.height as f32;
            pass.set_viewport(self.offset[0], self.offset[1], w, h, 0.0, 1.0);
            pass.set_scissor_rect(
                self.offset[0] as u32,
                self.offset[1] as u32,
                self.width,
                self.height,
            );
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.draw(0..6, 0..self.point_count);
        }
    }
}
