// Copied verbatim from j:/Projects/DragonSci/src/shaders/points.wgsl.
// Billboard point shader — renders each point instance as a screen-space quad.

struct Uniforms {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    // style: 0 = circle (soft), 1 = square, 2 = gaussian
    style: u32,
    // Negative means use the packed per-point size.
    point_size: f32,
    // Top-left, top-right, bottom-right, bottom-left viewport clip radii.
    clip_radii: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) alpha: f32,
    @location(3) local_position: vec2<f32>,
}

// Triangle strip forming a unit quad centered at origin.
var<private> QUAD: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(-0.5, -0.5),
    vec2<f32>( 0.5, -0.5),
    vec2<f32>(-0.5,  0.5),
    vec2<f32>( 0.5,  0.5),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    @location(0) position: vec3<f32>,
    @location(1) size: f32,
    @location(2) color: vec3<f32>,
    @location(3) alpha: f32,
) -> VertexOutput {
    let quad = QUAD[vid % 4u];
    let clip_center = uniforms.view_proj * vec4<f32>(position, 1.0);

    // Billboard offset in clip space: keeps size constant in pixels
    let effective_size = select(size, uniforms.point_size, uniforms.point_size >= 0.0);
    let ndc_offset = quad * effective_size / uniforms.screen_size * 2.0;
    let clip_offset = vec4<f32>(ndc_offset * clip_center.w, 0.0, 0.0);
    let center_ndc = clip_center.xyz / clip_center.w;
    let center_px = vec2<f32>(
        (center_ndc.x * 0.5 + 0.5) * uniforms.screen_size.x,
        (0.5 - center_ndc.y * 0.5) * uniforms.screen_size.y,
    );

    var out: VertexOutput;
    out.clip_position = clip_center + clip_offset;
    out.color = color;
    out.uv = quad + vec2<f32>(0.5);
    out.alpha = alpha;
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
        // square — full quad, no discard
    } else if uniforms.style == 2u {
        // gaussian — soft falloff, no hard edge
        a *= exp(-8.0 * dist * dist);
        if a < 0.004 { discard; }
    } else {
        // circle (default) — hard edge with 1-pixel AA
        if dist > 0.5 { discard; }
        a *= 1.0 - smoothstep(0.38, 0.50, dist);
    }
    return vec4<f32>(in.color, a);
}
