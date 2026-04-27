use std::collections::{HashMap, HashSet};
use std::path::Path;

use bytemuck::{Pod, Zeroable};

use crate::document::{WidgetKind, WidgetNode};
use crate::layout::{LayoutResult, Rect};
use crate::style::BORDER_WIDTH_LP;
use crate::theme::Theme;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ImageInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    radii: [f32; 4],
}

static IMAGE_ATTRS: [wgpu::VertexAttribute; 3] = [
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
];

fn instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ImageInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &IMAGE_ATTRS,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

struct ImageResource {
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct ImageDraw {
    path: String,
}

#[derive(Copy, Clone)]
enum ImageFit {
    Contain,
    Cover,
    Stretch,
}

impl ImageFit {
    fn from_node(node: &WidgetNode) -> Self {
        match node.props.image_fit.as_deref() {
            Some("cover") => Self::Cover,
            Some("stretch") => Self::Stretch,
            _ => Self::Contain,
        }
    }
}

pub struct ImageRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instance_buffer: wgpu::Buffer,
    instance_cap: u64,
    instances: Vec<ImageInstance>,
    draws: Vec<ImageDraw>,
    images: HashMap<String, ImageResource>,
    failed_paths: HashSet<String>,
}

impl ImageRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image-widget"),
            source: wgpu::ShaderSource::Wgsl(include_str!("image_widget.wgsl").into()),
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image-uniform-bgl"),
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

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("image-texture-bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("image-pipeline-layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("image-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[instance_layout()],
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
            label: Some("image-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image-uniform-bg"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let initial_cap = (8 * std::mem::size_of::<ImageInstance>()) as u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("image-instances"),
            size: initial_cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let renderer = Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,
            sampler,
            instance_buffer,
            instance_cap: initial_cap,
            instances: Vec::with_capacity(8),
            draws: Vec::new(),
            images: HashMap::new(),
            failed_paths: HashSet::new(),
        };
        renderer.update_screen_size(queue, width, height);
        renderer
    }

    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    pub fn forget_path(&mut self, path: &str) {
        self.images.remove(path);
        self.failed_paths.remove(path);
    }

    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree: &WidgetNode,
        layout: &LayoutResult,
        theme: &Theme,
        sf: f32,
    ) {
        let mut specs = Vec::new();
        collect_image_specs(tree, layout, theme, sf, &mut specs);

        let active_paths: HashSet<String> = specs.iter().map(|spec| spec.path.clone()).collect();
        self.images.retain(|path, _| active_paths.contains(path));
        self.failed_paths.retain(|path| active_paths.contains(path));

        for spec in &specs {
            if self.failed_paths.contains(&spec.path) && Path::new(&spec.path).exists() {
                self.failed_paths.remove(&spec.path);
            }
            if !self.images.contains_key(&spec.path) && !self.failed_paths.contains(&spec.path) {
                match load_image_resource(
                    device,
                    queue,
                    &self.texture_bind_group_layout,
                    &self.sampler,
                    &spec.path,
                ) {
                    Ok(resource) => {
                        self.images.insert(spec.path.clone(), resource);
                    }
                    Err(err) => {
                        eprintln!("DragonGUI: failed to load image {:?}: {err}", spec.path);
                        self.failed_paths.insert(spec.path.clone());
                    }
                }
            }
        }

        self.instances.clear();
        self.draws.clear();
        for spec in specs {
            let Some(resource) = self.images.get(&spec.path) else {
                continue;
            };
            let (rect, uv) = fit_rect_and_uv(spec.rect, resource.width, resource.height, spec.fit);
            if rect[2] <= 0.0 || rect[3] <= 0.0 {
                continue;
            }
            let radii = spec
                .radii
                .map(|radius| radius.min(rect[2] * 0.5).min(rect[3] * 0.5));
            self.instances.push(ImageInstance { rect, uv, radii });
            self.draws.push(ImageDraw { path: spec.path });
        }

        if self.instances.is_empty() {
            return;
        }

        let size = (self.instances.len() * std::mem::size_of::<ImageInstance>()) as u64;
        if size > self.instance_cap {
            let cap = (size * 2).max(1024);
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("image-instances"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_cap = cap;
        }
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.instances.is_empty() {
            return;
        }
        let stride = std::mem::size_of::<ImageInstance>() as u64;
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        for (index, draw) in self.draws.iter().enumerate() {
            let Some(resource) = self.images.get(&draw.path) else {
                continue;
            };
            pass.set_bind_group(1, &resource.bind_group, &[]);
            let start = index as u64 * stride;
            let end = start + stride;
            pass.set_vertex_buffer(0, self.instance_buffer.slice(start..end));
            pass.draw(0..6, 0..1);
        }
    }
}

struct ImageSpec {
    path: String,
    rect: Rect,
    fit: ImageFit,
    radii: [f32; 4],
}

fn collect_image_specs(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    out: &mut Vec<ImageSpec>,
) {
    if node.kind == WidgetKind::Image {
        if let (Some(path), Some(rect)) = (
            node.props.image_path.as_ref(),
            layout.visible_rect(&node.id),
        ) {
            let border_w = node
                .style
                .visual
                .border_width
                .unwrap_or(BORDER_WIDTH_LP)
                .max(0.0)
                * sf;
            let content = inset_rect(rect, border_w);
            let radius = node
                .style
                .visual
                .border_radius
                .unwrap_or(theme.radius)
                .max(0.0);
            let radii = node
                .style
                .visual
                .corner_radii
                .resolve(radius)
                .map(|radius| (radius.max(0.0) * sf - border_w).max(0.0));
            out.push(ImageSpec {
                path: path.clone(),
                rect: content,
                fit: ImageFit::from_node(node),
                radii,
            });
        }
    }
    for child in &node.children {
        collect_image_specs(child, layout, theme, sf, out);
    }
}

fn inset_rect(rect: Rect, inset: f32) -> Rect {
    Rect {
        x: rect.x + inset,
        y: rect.y + inset,
        w: (rect.w - inset * 2.0).max(0.0),
        h: (rect.h - inset * 2.0).max(0.0),
    }
}

fn load_image_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    path: &str,
) -> Result<ImageResource, String> {
    let image = image::open(Path::new(path)).map_err(|err| err.to_string())?;
    let rgba = image.to_rgba8();
    let width = rgba.width().max(1);
    let height = rgba.height().max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("image-texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("image-texture-bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    Ok(ImageResource {
        bind_group,
        width,
        height,
    })
}

fn fit_rect_and_uv(rect: Rect, image_w: u32, image_h: u32, fit: ImageFit) -> ([f32; 4], [f32; 4]) {
    let slot = [rect.x, rect.y, rect.w.max(0.0), rect.h.max(0.0)];
    if slot[2] <= 0.0 || slot[3] <= 0.0 || image_w == 0 || image_h == 0 {
        return (slot, [0.0, 0.0, 1.0, 1.0]);
    }

    let image_aspect = image_w as f32 / image_h as f32;
    let slot_aspect = slot[2] / slot[3];

    match fit {
        ImageFit::Stretch => (slot, [0.0, 0.0, 1.0, 1.0]),
        ImageFit::Contain => {
            let (draw_w, draw_h) = if image_aspect > slot_aspect {
                (slot[2], slot[2] / image_aspect)
            } else {
                (slot[3] * image_aspect, slot[3])
            };
            (
                [
                    slot[0] + (slot[2] - draw_w) * 0.5,
                    slot[1] + (slot[3] - draw_h) * 0.5,
                    draw_w,
                    draw_h,
                ],
                [0.0, 0.0, 1.0, 1.0],
            )
        }
        ImageFit::Cover => {
            if image_aspect > slot_aspect {
                let uv_w = slot_aspect / image_aspect;
                (slot, [(1.0 - uv_w) * 0.5, 0.0, uv_w, 1.0])
            } else {
                let uv_h = image_aspect / slot_aspect;
                (slot, [0.0, (1.0 - uv_h) * 0.5, 1.0, uv_h])
            }
        }
    }
}
