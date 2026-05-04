struct Uniforms {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    style: u32,
    point_size: f32,
    clip_radii: vec4<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_position: vec2<f32>,
}

var<private> CLIP_POSITIONS: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 1.0,  1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 1.0, -1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VertexOutput {
    let pos = CLIP_POSITIONS[vid % 4u];
    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.local_position = vec2<f32>(
        (pos.x * 0.5 + 0.5) * uniforms.screen_size.x,
        (0.5 - pos.y * 0.5) * uniforms.screen_size.y,
    );
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
    if rounded_clip_alpha(in.local_position, uniforms.screen_size, uniforms.clip_radii) <= 0.001 {
        discard;
    }
    return vec4<f32>(0.0);
}
