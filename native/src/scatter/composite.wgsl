struct Uniforms {
    target_rect: vec4<f32>,
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var source_texture: texture_2d<f32>;
@group(0) @binding(2) var source_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

var<private> QUAD: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    let p = QUAD[vid % 4u];
    let px = uniforms.target_rect.xy + p * uniforms.target_rect.zw;
    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x * 2.0 - 1.0,
        1.0 - px.y / uniforms.screen_size.y * 2.0,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = p;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(source_texture, source_sampler, in.uv);
}
