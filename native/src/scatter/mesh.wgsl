// Mesh pipeline for statistical overlays (convex hulls, ellipsoids).
// Supports solid (triangle-list) and wireframe (line-list) rendering.
// Alpha blending is handled by the render pass; depth writes are OFF for
// translucent meshes so points remain visible through overlays.

struct Uniforms {
    view_proj: mat4x4<f32>,
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) color:    vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       color:    vec4<f32>,
}

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_pos = u.view_proj * vec4<f32>(v.position, 1.0);
    out.color    = v.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
