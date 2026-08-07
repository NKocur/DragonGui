// Compact xyz_f32_v0 billboard point shader.
//
// This renders XYZ-only instance buffers directly. Size, alpha, and z-colormap
// color are derived from uniforms so native streaming can avoid expanding each
// point into the full 32-byte PointInstance layout before upload.

struct Uniforms {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    // style: 0 = circle (soft), 1 = square, 2 = gaussian
    style: u32,
    // Negative means use the compact default point size.
    point_size: f32,
    point_size_scale: f32,
    _pad0a: f32,
    _pad0b: f32,
    _pad0c: f32,
    clip_radii: vec4<f32>,
    compact_z_range: vec2<f32>,
    compact_colormap_len: u32,
    point_color_mode: u32,
    compact_colormap: array<vec4<f32>, 9>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) alpha: f32,
    @location(3) local_position: vec2<f32>,
}

var<private> QUAD: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(-0.5, -0.5),
    vec2<f32>( 0.5, -0.5),
    vec2<f32>(-0.5,  0.5),
    vec2<f32>( 0.5,  0.5),
);

fn sample_colormap(t_in: f32) -> vec3<f32> {
    let t = clamp(t_in, 0.0, 1.0);
    let len = max(uniforms.compact_colormap_len, 2u);
    let last = len - 1u;
    let scaled = t * f32(last);
    let i = min(u32(scaled), last - 1u);
    let frac = scaled - f32(i);
    let a = uniforms.compact_colormap[i].rgb;
    let b = uniforms.compact_colormap[i + 1u].rgb;
    return mix(a, b, frac);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    @location(0) position: vec3<f32>,
) -> VertexOutput {
    let quad = QUAD[vid % 4u];
    let clip_center = uniforms.view_proj * vec4<f32>(position, 1.0);

    let compact_default_size = 3.0;
    let effective_size = select(compact_default_size, uniforms.point_size, uniforms.point_size >= 0.0) * uniforms.point_size_scale;
    let ndc_offset = quad * effective_size / uniforms.screen_size * 2.0;
    let clip_offset = vec4<f32>(ndc_offset * clip_center.w, 0.0, 0.0);
    let center_ndc = clip_center.xyz / clip_center.w;
    let center_px = vec2<f32>(
        (center_ndc.x * 0.5 + 0.5) * uniforms.screen_size.x,
        (0.5 - center_ndc.y * 0.5) * uniforms.screen_size.y,
    );

    let z_min = uniforms.compact_z_range.x;
    let z_max = uniforms.compact_z_range.y;
    let z_range = max(z_max - z_min, 1.0);
    let z_is_finite = position.z == position.z && abs(position.z) < 3.402823e38;
    let t = select(0.0, clamp((position.z - z_min) / z_range, 0.0, 1.0), z_is_finite);

    var out: VertexOutput;
    out.clip_position = clip_center + clip_offset;
    out.color = sample_colormap(t);
    out.uv = quad + vec2<f32>(0.5);
    out.alpha = 1.0;
    out.local_position = center_px + quad * effective_size;
    return out;
}

fn rounded_clip_alpha(local_position: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let max_radius = max(max(radii.x, radii.y), max(radii.z, radii.w));
    if max_radius <= 0.0 {
        return 1.0;
    }

    let half_size = size * 0.5;
    let centered = local_position - half_size;
    let top_radius = select(radii.x, radii.y, centered.x > 0.0);
    let bottom_radius = select(radii.w, radii.z, centered.x > 0.0);
    let radius = min(select(top_radius, bottom_radius, centered.y > 0.0), min(half_size.x, half_size.y));
    if radius <= 0.0 {
        return 1.0;
    }

    let q = abs(centered) - (half_size - vec2<f32>(radius));
    let dist = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
    return 1.0 - smoothstep(-0.75, 0.75, dist);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let clip_alpha = rounded_clip_alpha(in.local_position, uniforms.screen_size, uniforms.clip_radii);
    if clip_alpha <= 0.001 { discard; }

    let dist = length(in.uv - vec2<f32>(0.5));
    var a = in.alpha * clip_alpha;
    if uniforms.style == 1u {
    } else if uniforms.style == 2u {
        a *= exp(-8.0 * dist * dist);
        if a < 0.004 { discard; }
    } else {
        if dist > 0.5 { discard; }
        a *= 1.0 - smoothstep(0.38, 0.50, dist);
    }
    return vec4<f32>(in.color, a);
}
