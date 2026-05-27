struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct SimpleRectInstance {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) clip_bounds: vec4<f32>,
}

struct VertOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_px: vec2<f32>,
    @location(2) @interpolate(flat) clip_bounds: vec4<f32>,
}

const QUAD: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    inst: SimpleRectInstance,
) -> VertOut {
    let uv = QUAD[vid];
    let px = inst.rect.xy + uv * inst.rect.zw;
    let ndc = vec2<f32>(
        px.x / u.screen_size.x * 2.0 - 1.0,
        -px.y / u.screen_size.y * 2.0 + 1.0,
    );
    var out: VertOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.color = inst.color;
    out.local_px = uv * inst.rect.zw;
    out.clip_bounds = inst.clip_bounds;
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    if (
        in.local_px.x < in.clip_bounds.x ||
        in.local_px.y < in.clip_bounds.y ||
        in.local_px.x > in.clip_bounds.z ||
        in.local_px.y > in.clip_bounds.w
    ) {
        discard;
    }
    if (in.color.a < 0.001) {
        discard;
    }
    return in.color;
}
