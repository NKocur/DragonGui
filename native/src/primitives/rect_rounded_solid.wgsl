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
    @location(2) @interpolate(flat) size: vec2<f32>,
    @location(3) @interpolate(flat) radii: vec4<f32>,
    @location(4) @interpolate(flat) clip_bounds: vec4<f32>,
    @location(5) @interpolate(flat) softness: f32,
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
    let aa_pad = 1.0;
    let uv = QUAD[vid];
    let expanded_size = inst.rect.zw + vec2<f32>(aa_pad * 2.0, aa_pad * 2.0);
    let px = inst.rect.xy - vec2<f32>(aa_pad, aa_pad) + uv * expanded_size;
    let ndc = vec2<f32>(
        px.x / u.screen_size.x * 2.0 - 1.0,
        -px.y / u.screen_size.y * 2.0 + 1.0,
    );
    var out: VertOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.color = inst.color;
    out.local_px = uv * expanded_size - vec2<f32>(aa_pad, aa_pad);
    out.size = inst.rect.zw;
    out.radii = inst.radii;
    out.clip_bounds = inst.clip_bounds;
    out.softness = 1.0;
    return out;
}

fn corner_radius(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let max_r = min(half_size.x, half_size.y);
    let tl = clamp(radii.x, 0.0, max_r);
    let tr = clamp(radii.y, 0.0, max_r);
    let br = clamp(radii.z, 0.0, max_r);
    let bl = clamp(radii.w, 0.0, max_r);
    if (p.x < 0.0) {
        if (p.y < 0.0) {
            return tl;
        }
        return bl;
    }
    if (p.y < 0.0) {
        return tr;
    }
    return br;
}

fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let r = corner_radius(p, half_size, radii);
    let d = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(d, vec2<f32>(0.0, 0.0))) + min(max(d.x, d.y), 0.0) - r;
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

    let half_size = in.size * 0.5;
    let p = in.local_px - half_size;
    let dist = rounded_rect_sdf(p, half_size, in.radii);
    let alpha = clamp(0.5 - dist / in.softness, 0.0, 1.0);
    if (alpha <= 0.001) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
