// Line-list pipeline for scatter grid, tick marks, and overlays.
// Shares the same uniform buffer as points.wgsl (only view_proj is consumed).

struct Uniforms {
    view_proj: mat4x4<f32>,
    // remaining fields from the shared uniform buffer are ignored
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color:    vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       color:    vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = u.view_proj * vec4<f32>(in.position, 1.0);
    out.color    = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
