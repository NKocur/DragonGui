// Fast path for solid transformed capsule line segments.
//
// LinePlot produces many solid rotated capsules. This shader keeps that path
// compact while preserving the same 1 px anti-aliased caps as the full rect
// primitive shader.

struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct LineSegmentInstance {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) params: vec4<f32>,
}

struct VertOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_px: vec2<f32>,
    @location(2) @interpolate(flat) half_size: vec2<f32>,
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
fn vs_main(@builtin(vertex_index) vid: u32, inst: LineSegmentInstance) -> VertOut {
    let aa_pad = 1.0;
    let uv = QUAD[vid];
    let expanded_size = inst.rect.zw + vec2<f32>(aa_pad * 2.0, aa_pad * 2.0);
    let px = inst.rect.xy - vec2<f32>(aa_pad, aa_pad) + uv * expanded_size;
    let center = inst.rect.xy + inst.rect.zw * 0.5;
    let local = px - center;
    let angle = inst.params.x;
    let c = cos(angle);
    let s = sin(angle);
    let rotated = vec2<f32>(
        local.x * c - local.y * s,
        local.x * s + local.y * c,
    );
    let screen_px = center + rotated;
    let ndc = vec2<f32>(
        screen_px.x / u.screen_size.x * 2.0 - 1.0,
        -screen_px.y / u.screen_size.y * 2.0 + 1.0,
    );

    var out: VertOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.color = inst.color;
    out.local_px = uv * expanded_size - vec2<f32>(aa_pad, aa_pad);
    out.half_size = inst.rect.zw * 0.5;
    return out;
}

fn capsule_sdf(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let radius = max(half_size.y, 0.5);
    let half_len = max(half_size.x - radius, 0.0);
    let q = vec2<f32>(abs(p.x) - half_len, p.y);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    let p = in.local_px - in.half_size;
    let sdf = capsule_sdf(p, max(in.half_size, vec2<f32>(0.5, 0.5)));
    let a = clamp(1.0 - sdf, 0.0, 1.0) * in.color.a;
    if (a < 0.001) {
        discard;
    }
    return vec4<f32>(in.color.rgb, a);
}
