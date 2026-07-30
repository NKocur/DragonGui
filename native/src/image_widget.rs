use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::path::Path;

use bytemuck::{Pod, Zeroable};
use image::ImageReader;
use serde_json::Value;

use crate::document::{WidgetKind, WidgetNode};
use crate::events::WidgetState;
use crate::layout::{LayoutResult, Rect};
use crate::primitives::visual_for as visual_for_widget;
use crate::resources::ResourceRegistry;
use crate::style::{
    BackgroundImage, BackgroundImageFit, BackgroundPaint, PositionStyle, TransformStyle,
    BORDER_WIDTH_LP,
};
use crate::theme::Theme;

const MAX_MANAGED_IMAGE_DIMENSION: u32 = 4096;
const MAX_MANAGED_IMAGE_PIXELS: u64 = 16_000_000;
const MAX_MANAGED_IMAGE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ImageInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    radii: [f32; 4],
    transform: [f32; 4],
    transform2: [f32; 4],
    clip: [f32; 4],
    params: [f32; 4],
    fallback: [f32; 4],
}

static IMAGE_ATTRS: [wgpu::VertexAttribute; 8] = [
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
    repeat_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    version: Option<u64>,
}

#[derive(Clone)]
struct ImageDraw {
    key: String,
    repeat: bool,
    background: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ImageFit {
    Contain,
    Cover,
    Stretch,
    Repeat,
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
    repeat_sampler: wgpu::Sampler,
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
        let repeat_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image-repeat-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
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
            repeat_sampler,
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
        let key = path_cache_key(path);
        self.images.remove(&key);
        self.failed_paths.remove(&key);
    }

    pub fn rebuild(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tree: &WidgetNode,
        layout: &LayoutResult,
        theme: &Theme,
        sf: f32,
        state: Option<&WidgetState>,
        resources: &ResourceRegistry,
    ) {
        let mut specs = Vec::new();
        collect_image_specs(tree, layout, theme, sf, state, &mut specs);

        let active_keys: HashSet<String> = specs.iter().map(ImageSpec::cache_key).collect();
        self.images.retain(|key, _| active_keys.contains(key));
        self.failed_paths.retain(|key| active_keys.contains(key));

        for spec in &specs {
            let key = spec.cache_key();
            let managed = spec
                .resource_id
                .as_deref()
                .and_then(|id| resources.image(id));
            let expected_version = managed.map(|resource| resource.version);
            let stale = self
                .images
                .get(&key)
                .is_some_and(|image| image.version != expected_version);
            if stale {
                self.images.remove(&key);
            }
            if spec
                .path
                .as_ref()
                .is_some_and(|path| Path::new(path).exists())
            {
                self.failed_paths.remove(&key);
            }
            if managed.is_some() {
                self.failed_paths.remove(&key);
            }
            if !self.images.contains_key(&key) && !self.failed_paths.contains(&key) {
                let loaded = if let Some(resource) = managed {
                    load_managed_image_resource(
                        device,
                        queue,
                        &self.texture_bind_group_layout,
                        &self.sampler,
                        &self.repeat_sampler,
                        &resource.bytes,
                        resource.version,
                    )
                } else if spec.resource_id.is_some() {
                    load_transparent_image_resource(
                        device,
                        queue,
                        &self.texture_bind_group_layout,
                        &self.sampler,
                        &self.repeat_sampler,
                        None,
                    )
                } else if let Some(path) = spec.path.as_deref() {
                    load_path_image_resource(
                        device,
                        queue,
                        &self.texture_bind_group_layout,
                        &self.sampler,
                        &self.repeat_sampler,
                        path,
                    )
                } else {
                    continue;
                };
                match loaded {
                    Ok(resource) => {
                        self.images.insert(key, resource);
                    }
                    Err(err) => {
                        eprintln!(
                            "DragonGUI: failed to load image {:?}: {err}",
                            spec.source_name()
                        );
                        if spec.resource_id.is_some() {
                            if let Ok(resource) = load_transparent_image_resource(
                                device,
                                queue,
                                &self.texture_bind_group_layout,
                                &self.sampler,
                                &self.repeat_sampler,
                                expected_version,
                            ) {
                                self.images.insert(key.clone(), resource);
                            }
                        }
                        self.failed_paths.insert(key);
                    }
                }
            }
        }

        self.instances.clear();
        self.draws.clear();
        for spec in specs {
            let key = spec.cache_key();
            let Some(resource) = self.images.get(&key) else {
                continue;
            };
            let (rect, uv) = fit_rect_and_uv(spec.rect, resource.width, resource.height, spec.fit);
            if rect[2] <= 0.0 || rect[3] <= 0.0 {
                continue;
            }
            let radii = spec
                .radii
                .map(|radius| radius.min(rect[2] * 0.5).min(rect[3] * 0.5));
            let (transform, transform2) = encoded_transform(spec.transform, sf);
            self.instances.push(ImageInstance {
                rect,
                uv,
                radii,
                transform,
                transform2,
                clip: [spec.clip.x, spec.clip.y, spec.clip.w, spec.clip.h],
                params: [
                    spec.opacity.clamp(0.0, 1.0),
                    if spec.fit == ImageFit::Contain {
                        1.0
                    } else {
                        0.0
                    },
                    0.0,
                    0.0,
                ],
                fallback: spec.fallback,
            });
            self.draws.push(ImageDraw {
                key,
                repeat: spec.fit == ImageFit::Repeat,
                background: spec.background,
            });
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

    pub fn render_backgrounds(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.render_layer(pass, true);
    }

    pub fn render_content(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.render_layer(pass, false);
    }

    fn render_layer(&self, pass: &mut wgpu::RenderPass<'_>, background: bool) {
        if self.instances.is_empty() {
            return;
        }
        let stride = std::mem::size_of::<ImageInstance>() as u64;
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        for (index, draw) in self.draws.iter().enumerate() {
            if draw.background != background {
                continue;
            }
            let Some(resource) = self.images.get(&draw.key) else {
                continue;
            };
            let bind_group = if draw.repeat {
                &resource.repeat_bind_group
            } else {
                &resource.bind_group
            };
            pass.set_bind_group(1, bind_group, &[]);
            let start = index as u64 * stride;
            let end = start + stride;
            pass.set_vertex_buffer(0, self.instance_buffer.slice(start..end));
            pass.draw(0..6, 0..1);
        }
    }
}

struct ImageSpec {
    path: Option<String>,
    resource_id: Option<String>,
    rect: Rect,
    clip: Rect,
    fit: ImageFit,
    radii: [f32; 4],
    transform: Option<TransformStyle>,
    opacity: f32,
    background: bool,
    fallback: [f32; 4],
}

impl ImageSpec {
    fn cache_key(&self) -> String {
        if let Some(id) = &self.resource_id {
            resource_cache_key(id)
        } else {
            path_cache_key(self.path.as_deref().unwrap_or_default())
        }
    }

    fn source_name(&self) -> &str {
        self.resource_id
            .as_deref()
            .or(self.path.as_deref())
            .unwrap_or("<missing>")
    }
}

fn path_cache_key(path: &str) -> String {
    format!("path:{path}")
}

fn resource_cache_key(id: &str) -> String {
    format!("resource:{id}")
}

fn value_f32(value: &Value) -> Option<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
}

fn object_f32(map: &serde_json::Map<String, Value>, name: &str) -> Option<f32> {
    map.get(name).and_then(value_f32)
}

fn display_list_scale(node: &WidgetNode, rect: Rect) -> (f32, f32) {
    let paint_w = node
        .props
        .raw_props
        .get("paint_width")
        .and_then(value_f32)
        .filter(|value| *value > 0.0)
        .or(node.props.intrinsic_width)
        .unwrap_or(rect.w.max(1.0));
    let paint_h = node
        .props
        .raw_props
        .get("paint_height")
        .and_then(value_f32)
        .filter(|value| *value > 0.0)
        .or(node.props.intrinsic_height)
        .unwrap_or(rect.h.max(1.0));
    (rect.w / paint_w.max(1.0), rect.h / paint_h.max(1.0))
}

fn image_fit_from_value(value: Option<&Value>) -> ImageFit {
    match value.and_then(Value::as_str).unwrap_or("contain") {
        "cover" => ImageFit::Cover,
        "stretch" => ImageFit::Stretch,
        _ => ImageFit::Contain,
    }
}

fn collect_extension_display_list_image_specs(
    node: &WidgetNode,
    layout: &LayoutResult,
    out: &mut Vec<ImageSpec>,
) {
    if node.kind != WidgetKind::Extension {
        return;
    }
    let Some(Value::Array(commands)) = node.props.raw_props.get("display_list") else {
        return;
    };
    let Some(rect) = layout.rects.get(&node.id).copied() else {
        return;
    };
    if rect.w <= 0.0 || rect.h <= 0.0 || layout.visible_rect(&node.id).is_none() {
        return;
    }
    let (sx, sy) = display_list_scale(node, rect);
    for command in commands {
        let Some(command) = command.as_object() else {
            continue;
        };
        let cmd = command
            .get("cmd")
            .or_else(|| command.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if cmd != "image" {
            continue;
        }
        let Some(path) = command
            .get("path")
            .or_else(|| command.get("src"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(local_x) = object_f32(command, "x") else {
            continue;
        };
        let Some(local_y) = object_f32(command, "y") else {
            continue;
        };
        let Some(local_w) = object_f32(command, "w").or_else(|| object_f32(command, "width"))
        else {
            continue;
        };
        let Some(local_h) = object_f32(command, "h").or_else(|| object_f32(command, "height"))
        else {
            continue;
        };
        if local_w <= 0.0 || local_h <= 0.0 {
            continue;
        }
        let image_rect = Rect {
            x: rect.x + local_x * sx,
            y: rect.y + local_y * sy,
            w: local_w * sx,
            h: local_h * sy,
        };
        let radius = object_f32(command, "radius").unwrap_or(0.0).max(0.0) * sx.min(sy).abs();
        out.push(ImageSpec {
            path: Some(path.to_string()),
            resource_id: None,
            rect: image_rect,
            clip: layout.visible_rect(&node.id).unwrap_or(rect),
            fit: image_fit_from_value(command.get("fit")),
            radii: [radius; 4],
            transform: None,
            opacity: 1.0,
            background: false,
            fallback: [0.0; 4],
        });
    }
}

fn collect_image_specs(
    node: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    sf: f32,
    state: Option<&WidgetState>,
    out: &mut Vec<ImageSpec>,
) {
    let subtree_start = out.len();
    if let (Some(rect), Some(clip)) = (
        layout.rects.get(&node.id).copied(),
        layout.visible_rect(&node.id),
    ) {
        let visual = state
            .map(|state| visual_for_widget(node, state, theme))
            .unwrap_or_else(|| Cow::Borrowed(&node.style.visual));
        if let Some(image) = background_image(&visual.background_paint) {
            let border_w = visual.border_width.unwrap_or(BORDER_WIDTH_LP).max(0.0) * sf;
            let content = inset_rect(rect, border_w);
            let radius = visual.border_radius.unwrap_or(theme.radius).max(0.0);
            let radii = visual
                .corner_radii
                .resolve(radius)
                .map(|radius| (radius.max(0.0) * sf - border_w).max(0.0));
            out.push(ImageSpec {
                path: None,
                resource_id: Some(image.resource_id.clone()),
                rect: content,
                clip,
                fit: background_image_fit(image.fit),
                radii,
                transform: None,
                opacity: visual.opacity.unwrap_or(1.0),
                background: true,
                fallback: background_fallback(
                    &visual.background_paint,
                    visual.background.as_ref(),
                    theme,
                ),
            });
        }
    }
    if matches!(node.kind, WidgetKind::Image | WidgetKind::ImageButton) {
        if let (Some(path), Some(rect), Some(clip)) = (
            node.props.image_path.as_ref(),
            layout.rects.get(&node.id).copied(),
            layout.visible_rect(&node.id),
        ) {
            let visual = state
                .map(|state| visual_for_widget(node, state, theme))
                .unwrap_or_else(|| Cow::Borrowed(&node.style.visual));
            let border_w = visual.border_width.unwrap_or(BORDER_WIDTH_LP).max(0.0) * sf;
            let button_inset = if node.kind == WidgetKind::ImageButton {
                node.style.layout.padding.unwrap_or(5.0).max(0.0) * sf
            } else {
                0.0
            };
            let content = inset_rect(rect, border_w + button_inset);
            let radius = visual.border_radius.unwrap_or(theme.radius).max(0.0);
            let radii = visual
                .corner_radii
                .resolve(radius)
                .map(|radius| (radius.max(0.0) * sf - border_w).max(0.0));
            out.push(ImageSpec {
                path: Some(path.clone()),
                resource_id: None,
                rect: content,
                clip,
                fit: ImageFit::from_node(node),
                radii,
                transform: None,
                opacity: visual.opacity.unwrap_or(1.0),
                background: false,
                fallback: [0.0; 4],
            });
        }
    }
    collect_extension_display_list_image_specs(node, layout, out);
    for child in &node.children {
        collect_image_specs(child, layout, theme, sf, state, out);
    }
    if let Some(rect) = layout.rects.get(&node.id) {
        let visual = state
            .map(|state| visual_for_widget(node, state, theme))
            .unwrap_or_else(|| Cow::Borrowed(&node.style.visual));
        apply_transform_to_specs(
            &mut out[subtree_start..],
            paint_transform_for_node(node, visual.transform),
            sf,
            [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5],
        );
    }
}

fn background_image(paint: &Option<BackgroundPaint>) -> Option<&BackgroundImage> {
    match paint.as_ref()? {
        BackgroundPaint::Image(image) => Some(image),
        BackgroundPaint::Layers(layers) => layers.iter().find_map(|paint| match paint {
            BackgroundPaint::Image(image) => Some(image),
            _ => None,
        }),
        _ => None,
    }
}

fn background_image_fit(fit: BackgroundImageFit) -> ImageFit {
    match fit {
        BackgroundImageFit::Contain => ImageFit::Contain,
        BackgroundImageFit::Cover => ImageFit::Cover,
        BackgroundImageFit::Stretch => ImageFit::Stretch,
        BackgroundImageFit::Repeat => ImageFit::Repeat,
    }
}

fn background_fallback(
    paint: &Option<BackgroundPaint>,
    background: Option<&crate::style::ColorRef>,
    theme: &Theme,
) -> [f32; 4] {
    let from_layers = match paint {
        Some(BackgroundPaint::Layers(layers)) => layers.iter().find_map(|paint| match paint {
            BackgroundPaint::Color(color) => Some(color.resolve(theme)),
            _ => None,
        }),
        _ => None,
    };
    from_layers
        .or_else(|| background.map(|color| color.resolve(theme)))
        .unwrap_or([0.0; 4])
}

fn apply_transform_to_specs(
    specs: &mut [ImageSpec],
    transform: Option<TransformStyle>,
    sf: f32,
    origin: [f32; 2],
) {
    let Some(transform) = transform.filter(|transform| !transform.is_identity()) else {
        return;
    };
    for spec in specs {
        let composed = compose_transform_for_rect(spec.rect, spec.transform, transform, sf, origin);
        spec.transform = (!composed.is_identity()).then_some(composed);
    }
}

fn compose_transform_for_rect(
    rect: Rect,
    existing: Option<TransformStyle>,
    parent: TransformStyle,
    sf: f32,
    origin: [f32; 2],
) -> TransformStyle {
    let existing = existing.unwrap_or_default();
    let center = [rect.x + rect.w * 0.5, rect.y + rect.h * 0.5];
    let current_center = [
        center[0] + existing.translate_x * sf,
        center[1] + existing.translate_y * sf,
    ];
    let parent_scale = [parent.scale_x, parent.scale_y];
    let rotation = parent.rotate_deg.to_radians();
    let cos_r = rotation.cos();
    let sin_r = rotation.sin();
    let scaled = [
        (current_center[0] - origin[0]) * parent_scale[0],
        (current_center[1] - origin[1]) * parent_scale[1],
    ];
    let rotated = [
        scaled[0] * cos_r - scaled[1] * sin_r,
        scaled[0] * sin_r + scaled[1] * cos_r,
    ];
    let transformed_center = [
        origin[0] + rotated[0] + parent.translate_x * sf,
        origin[1] + rotated[1] + parent.translate_y * sf,
    ];
    TransformStyle {
        translate_x: (transformed_center[0] - center[0]) / sf,
        translate_y: (transformed_center[1] - center[1]) / sf,
        scale_x: existing.scale_x * parent.scale_x,
        scale_y: existing.scale_y * parent.scale_y,
        rotate_deg: existing.rotate_deg + parent.rotate_deg,
    }
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

fn encoded_transform(transform: Option<TransformStyle>, sf: f32) -> ([f32; 4], [f32; 4]) {
    let Some(transform) = transform.filter(|transform| !transform.is_identity()) else {
        return ([0.0, 0.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.0]);
    };
    (
        [
            transform.translate_x * sf,
            transform.translate_y * sf,
            transform.scale_x,
            transform.scale_y,
        ],
        [transform.rotate_deg.to_radians(), 0.0, 0.0, 0.0],
    )
}

fn inset_rect(rect: Rect, inset: f32) -> Rect {
    Rect {
        x: rect.x + inset,
        y: rect.y + inset,
        w: (rect.w - inset * 2.0).max(0.0),
        h: (rect.h - inset * 2.0).max(0.0),
    }
}

fn load_path_image_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    repeat_sampler: &wgpu::Sampler,
    path: &str,
) -> Result<ImageResource, String> {
    let image = image::open(Path::new(path)).map_err(|err| err.to_string())?;
    upload_image_resource(device, queue, layout, sampler, repeat_sampler, image, None)
}

fn load_managed_image_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    repeat_sampler: &wgpu::Sampler,
    bytes: &[u8],
    version: u64,
) -> Result<ImageResource, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_MANAGED_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_MANAGED_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_MANAGED_IMAGE_ALLOC_BYTES);
    reader.limits(limits);
    let image = reader.decode().map_err(|err| err.to_string())?;
    let pixels = u64::from(image.width()).saturating_mul(u64::from(image.height()));
    if image.width() > MAX_MANAGED_IMAGE_DIMENSION
        || image.height() > MAX_MANAGED_IMAGE_DIMENSION
        || pixels > MAX_MANAGED_IMAGE_PIXELS
    {
        return Err(format!(
            "managed image dimensions {}x{} exceed the 4096x4096 / 16M-pixel limit",
            image.width(),
            image.height()
        ));
    }
    upload_image_resource(
        device,
        queue,
        layout,
        sampler,
        repeat_sampler,
        image,
        Some(version),
    )
}

fn load_transparent_image_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    repeat_sampler: &wgpu::Sampler,
    version: Option<u64>,
) -> Result<ImageResource, String> {
    upload_image_resource(
        device,
        queue,
        layout,
        sampler,
        repeat_sampler,
        image::DynamicImage::new_rgba8(1, 1),
        version,
    )
}

fn upload_image_resource(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    repeat_sampler: &wgpu::Sampler,
    image: image::DynamicImage,
    version: Option<u64>,
) -> Result<ImageResource, String> {
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
    let repeat_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("image-repeat-texture-bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(repeat_sampler),
            },
        ],
    });
    Ok(ImageResource {
        bind_group,
        repeat_bind_group,
        width,
        height,
        version,
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
        ImageFit::Repeat => (
            slot,
            [0.0, 0.0, slot[2] / image_w as f32, slot[3] / image_h as f32],
        ),
        ImageFit::Contain => {
            let (draw_w, draw_h) = if image_aspect > slot_aspect {
                (slot[2], slot[2] / image_aspect)
            } else {
                (slot[3] * image_aspect, slot[3])
            };
            let draw_fraction_x = (draw_w / slot[2]).max(f32::EPSILON);
            let draw_fraction_y = (draw_h / slot[3]).max(f32::EPSILON);
            let offset_x = (1.0 - draw_fraction_x) * 0.5;
            let offset_y = (1.0 - draw_fraction_y) * 0.5;
            (
                slot,
                [
                    -offset_x / draw_fraction_x,
                    -offset_y / draw_fraction_y,
                    1.0 / draw_fraction_x,
                    1.0 / draw_fraction_y,
                ],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::NodeProps;

    fn node(id: &str, kind: WidgetKind) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name: None,
            css_types: Vec::new(),
            kind,
            props: NodeProps::default(),
            style_json: Default::default(),
            default_style: Default::default(),
            inline_style: Default::default(),
            style: Default::default(),
            children: Vec::new(),
        }
    }

    #[test]
    fn image_spec_carries_widget_transform_to_texture_instance() {
        let mut image = node("hero", WidgetKind::Image);
        image.props.image_path = Some("examples/hero.png".to_string());
        image.style.visual.border_width = Some(2.0);
        image.style.visual.transform = Some(TransformStyle {
            translate_x: 3.0,
            translate_y: -2.0,
            scale_x: 1.1,
            scale_y: 0.9,
            rotate_deg: 8.0,
        });

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "hero".to_string(),
            Rect {
                x: 10.0,
                y: 20.0,
                w: 100.0,
                h: 80.0,
            },
        );

        let mut specs = Vec::new();
        collect_image_specs(&image, &layout, &Theme::dark(), 2.0, None, &mut specs);

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].rect.x, 14.0);
        assert_eq!(specs[0].rect.y, 24.0);
        assert_eq!(specs[0].rect.w, 92.0);
        assert_eq!(specs[0].rect.h, 72.0);
        let (transform, transform2) = encoded_transform(specs[0].transform, 2.0);
        assert_eq!(transform, [6.0, -4.0, 1.1, 0.9]);
        assert!((transform2[0] - 8.0_f32.to_radians()).abs() < 0.001);
    }

    #[test]
    fn extension_display_list_image_emits_scaled_image_spec() {
        let mut extension = node("paint", WidgetKind::Extension);
        let props = serde_json::json!({
            "extension_type": "paint",
            "paint_width": 100,
            "paint_height": 50,
            "display_list": [
                {"cmd": "image", "path": "examples/logo.png", "x": 10, "y": 5, "w": 40, "h": 20, "fit": "cover", "radius": 3}
            ]
        });
        extension.props.raw_props = props.as_object().unwrap().clone();
        extension.props.extension_type = Some("paint".to_string());
        extension.props.intrinsic_width = Some(100.0);
        extension.props.intrinsic_height = Some(50.0);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "paint".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 100.0,
            },
        );

        let mut specs = Vec::new();
        collect_image_specs(&extension, &layout, &Theme::dark(), 1.0, None, &mut specs);

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].path.as_deref(), Some("examples/logo.png"));
        assert_eq!(specs[0].rect.x, 20.0);
        assert_eq!(specs[0].rect.y, 10.0);
        assert_eq!(specs[0].rect.w, 80.0);
        assert_eq!(specs[0].rect.h, 40.0);
        assert_eq!(specs[0].fit, ImageFit::Cover);
        assert_eq!(specs[0].radii, [6.0; 4]);
    }

    #[test]
    fn managed_background_spec_uses_resource_fit_clip_and_fallback() {
        let mut panel = node("surface", WidgetKind::Panel);
        panel.style.visual.background = Some(crate::style::ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]));
        panel.style.visual.background_paint = Some(BackgroundPaint::Layers(vec![
            BackgroundPaint::Image(BackgroundImage {
                resource_id: "linen".to_string(),
                fit: BackgroundImageFit::Repeat,
            }),
            BackgroundPaint::Color(crate::style::ColorRef::Rgba([0.1, 0.2, 0.3, 1.0])),
        ]));
        panel.style.visual.opacity = Some(0.75);

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "surface".to_string(),
            Rect {
                x: 10.0,
                y: 20.0,
                w: 120.0,
                h: 80.0,
            },
        );
        let mut specs = Vec::new();
        collect_image_specs(&panel, &layout, &Theme::dark(), 1.0, None, &mut specs);

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].resource_id.as_deref(), Some("linen"));
        assert_eq!(specs[0].fit, ImageFit::Repeat);
        assert!(specs[0].background);
        assert_eq!(specs[0].opacity, 0.75);
        assert_eq!(specs[0].fallback, [0.1, 0.2, 0.3, 1.0]);
        assert_eq!(specs[0].clip.w, 120.0);
    }

    #[test]
    fn managed_background_fit_uvs_are_bounded_and_repeatable() {
        let slot = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        let (repeat_rect, repeat_uv) = fit_rect_and_uv(slot, 10, 10, ImageFit::Repeat);
        assert_eq!(repeat_rect, [0.0, 0.0, 100.0, 50.0]);
        assert_eq!(repeat_uv, [0.0, 0.0, 10.0, 5.0]);

        let (contain_rect, contain_uv) = fit_rect_and_uv(slot, 10, 20, ImageFit::Contain);
        assert_eq!(contain_rect, [0.0, 0.0, 100.0, 50.0]);
        assert!(contain_uv[0] < 0.0);
        assert!(contain_uv[2] > 1.0);
    }

    #[test]
    fn image_spec_interpolates_state_visual_transform_for_texture_instance() {
        let mut image = node("hero", WidgetKind::Image);
        image.props.image_path = Some("examples/hero.png".to_string());
        image.style.hover.transform = Some(TransformStyle {
            translate_x: 0.0,
            translate_y: -4.0,
            scale_x: 1.04,
            scale_y: 1.04,
            rotate_deg: 2.0,
        });

        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "hero".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 80.0,
            },
        );
        let mut state = WidgetState::default();
        state.hovered = Some("hero".to_string());
        state.hover_t.insert("hero".to_string(), 0.5);

        let mut specs = Vec::new();
        collect_image_specs(
            &image,
            &layout,
            &Theme::dark(),
            1.5,
            Some(&state),
            &mut specs,
        );

        let (transform, transform2) = encoded_transform(specs[0].transform, 1.5);
        assert_eq!(transform, [0.0, -3.0, 1.02, 1.02]);
        assert!((transform2[0] - 1.0_f32.to_radians()).abs() < 0.001);
    }

    #[test]
    fn container_transform_propagates_to_child_image_texture_instance() {
        let mut panel = node("panel", WidgetKind::Panel);
        panel.style.visual.transform = Some(TransformStyle {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 2.0,
            scale_y: 2.0,
            rotate_deg: 0.0,
        });
        let mut image = node("hero", WidgetKind::Image);
        image.props.image_path = Some("examples/hero.png".to_string());
        image.style.visual.border_width = Some(0.0);
        panel.children.push(image);

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
            "hero".to_string(),
            Rect {
                x: 10.0,
                y: 10.0,
                w: 20.0,
                h: 20.0,
            },
        );

        let mut specs = Vec::new();
        collect_image_specs(&panel, &layout, &Theme::dark(), 1.0, None, &mut specs);

        let (transform, transform2) = encoded_transform(specs[0].transform, 1.0);
        assert_eq!(transform, [-30.0, -30.0, 2.0, 2.0]);
        assert_eq!(transform2[0], 0.0);
    }
}
