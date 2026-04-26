struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var image_texture: texture_2d<f32>;

@group(1) @binding(1)
var image_sampler: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: Instance) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let pixel = instance.rect.xy + corner * instance.rect.zw;
    let ndc = vec2<f32>(
        pixel.x / uniforms.screen_size.x * 2.0 - 1.0,
        1.0 - pixel.y / uniforms.screen_size.y * 2.0,
    );

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = instance.uv.xy + corner * instance.uv.zw;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(image_texture, image_sampler, in.uv);
}
