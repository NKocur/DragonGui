struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct LinePlotSegmentInstance {
    @location(0) start: vec2<f32>,
    @location(1) end: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params: vec4<f32>,
}

struct VertOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
    @location(2) @interpolate(flat) half_len: f32,
    @location(3) @interpolate(flat) radius: f32,
}

const QUAD: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, 1.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    inst: LinePlotSegmentInstance,
) -> VertOut {
    let delta = inst.end - inst.start;
    let len = max(length(delta), 0.001);
    let dir = delta / len;
    let normal = vec2<f32>(-dir.y, dir.x);
    let radius = max(inst.params.x * 0.5, 0.5);
    let aa_pad = 1.0;
    let half_len = len * 0.5;
    let center = (inst.start + inst.end) * 0.5;
    let uv = QUAD[vid];
    let local = vec2<f32>(uv.x * (half_len + radius + aa_pad), uv.y * (radius + aa_pad));
    let px = center + dir * local.x + normal * local.y;
    let ndc = vec2<f32>(
        px.x / u.screen_size.x * 2.0 - 1.0,
        -px.y / u.screen_size.y * 2.0 + 1.0,
    );

    var out: VertOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.color = inst.color;
    out.local = local;
    out.half_len = half_len;
    out.radius = radius;
    return out;
}

fn capsule_sdf(local: vec2<f32>, half_len: f32, radius: f32) -> f32 {
    let q = vec2<f32>(abs(local.x) - half_len, abs(local.y)) - vec2<f32>(0.0, radius);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0);
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    if (in.color.a < 0.001) {
        discard;
    }
    let dist = capsule_sdf(in.local, in.half_len, in.radius);
    let alpha = clamp(0.5 - dist, 0.0, 1.0);
    if (alpha <= 0.001) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
