// Instanced solid-colour rectangle renderer with optional rounded corners.
//
// Each instance specifies a pixel-space rect (x, y, w, h), an RGBA colour,
// and a corner radius in pixels.  The vertex shader expands the instance into
// a two-triangle quad; the fragment shader discards pixels outside the rounded
// corner arc using a signed-distance field.

struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct RectInstance {
    @location(0) rect:   vec4<f32>,  // x, y, w, h  (pixels, top-left origin)
    @location(1) color:  vec4<f32>,  // rgba linear
    @location(2) radius: f32,        // corner radius (pixels); 0 = sharp corners
}

struct VertOut {
    @builtin(position)            clip:      vec4<f32>,
    @location(0)                  color:     vec4<f32>,
    // Pixel coordinates within the rect, origin at the rect's top-left.
    @location(1)                  local_px:  vec2<f32>,
    // Half-size and radius are constant per instance — mark flat to avoid
    // interpolation artefacts at triangle edges.
    @location(2) @interpolate(flat) half_size: vec2<f32>,
    @location(3) @interpolate(flat) radius:    f32,
}

// Two-triangle unit quad (CCW).
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
    inst: RectInstance,
) -> VertOut {
    let uv        = QUAD[vid];
    let px        = inst.rect.xy + uv * inst.rect.zw;
    // pixel → NDC: x: [0, W] → [-1, 1];  y flipped: [0, H] → [1, -1]
    let ndc = vec2<f32>(
         px.x / u.screen_size.x * 2.0 - 1.0,
        -px.y / u.screen_size.y * 2.0 + 1.0,
    );
    var out: VertOut;
    out.clip      = vec4<f32>(ndc, 0.0, 1.0);
    out.color     = inst.color;
    out.local_px  = uv * inst.rect.zw;
    out.half_size = inst.rect.zw * 0.5;
    out.radius    = inst.radius;
    return out;
}

// Standard box-SDF with uniform corner radius.
fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let d = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(d, vec2<f32>(0.0, 0.0))) + min(max(d.x, d.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    // Clamp radius so it can never exceed the smaller half-dimension.
    let r   = clamp(in.radius, 0.0, min(in.half_size.x, in.half_size.y));
    let p   = in.local_px - in.half_size;          // centered coords
    let sdf = rounded_rect_sdf(p, in.half_size, r);
    // Anti-aliased edge: smoothly fade over 1 pixel.
    let a   = clamp(1.0 - sdf, 0.0, 1.0) * in.color.a;
    if a < 0.001 { discard; }
    return vec4<f32>(in.color.rgb, a);
}
