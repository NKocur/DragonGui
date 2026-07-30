struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var image_texture: texture_2d<f32>;

@group(1) @binding(1)
var image_sampler: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) transform: vec4<f32>, // x/y translation pixels, z/w scale
    @location(4) transform2: vec4<f32>, // x rotation radians
    @location(5) clip_rect: vec4<f32>,
    @location(6) params: vec4<f32>, // x opacity
    @location(7) fallback: vec4<f32>,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) local_px: vec2<f32>,
    @location(2) @interpolate(flat) half_size: vec2<f32>,
    @location(3) @interpolate(flat) radii: vec4<f32>,
    @location(4) screen_px: vec2<f32>,
    @location(5) @interpolate(flat) clip_rect: vec4<f32>,
    @location(6) @interpolate(flat) opacity: f32,
    @location(7) @interpolate(flat) contain: f32,
    @location(8) @interpolate(flat) fallback: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: Instance) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let pixel = instance.rect.xy + corner * instance.rect.zw;
    let center = instance.rect.xy + instance.rect.zw * 0.5;
    let scaled = (pixel - center) * instance.transform.zw;
    let angle = instance.transform2.x;
    let c = cos(angle);
    let s = sin(angle);
    let rotated = vec2<f32>(
        scaled.x * c - scaled.y * s,
        scaled.x * s + scaled.y * c,
    );
    let transformed_pixel = center + rotated + instance.transform.xy;
    let ndc = vec2<f32>(
        transformed_pixel.x / uniforms.screen_size.x * 2.0 - 1.0,
        1.0 - transformed_pixel.y / uniforms.screen_size.y * 2.0,
    );

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = instance.uv.xy + corner * instance.uv.zw;
    out.local_px = corner * instance.rect.zw;
    out.half_size = instance.rect.zw * 0.5;
    out.radii = instance.radii;
    out.screen_px = transformed_pixel;
    out.clip_rect = instance.clip_rect;
    out.opacity = instance.params.x;
    out.contain = instance.params.y;
    out.fallback = instance.fallback;
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
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let clip_end = in.clip_rect.xy + in.clip_rect.zw;
    if (in.screen_px.x < in.clip_rect.x || in.screen_px.y < in.clip_rect.y ||
        in.screen_px.x > clip_end.x || in.screen_px.y > clip_end.y) {
        discard;
    }
    let p = in.local_px - in.half_size;
    let sdf = rounded_rect_sdf(p, in.half_size, in.radii);
    let edge_alpha = clamp(1.0 - sdf, 0.0, 1.0);
    if (edge_alpha < 0.001) {
        discard;
    }
    let outside_image = in.uv.x < 0.0 || in.uv.y < 0.0 || in.uv.x > 1.0 || in.uv.y > 1.0;
    var tex = textureSample(image_texture, image_sampler, in.uv);
    if (in.contain > 0.5 && outside_image) {
        tex = vec4<f32>(0.0);
    }
    let alpha = tex.a + in.fallback.a * (1.0 - tex.a);
    var rgb = in.fallback.rgb;
    if (alpha > 0.0001) {
        rgb = (tex.rgb * tex.a + in.fallback.rgb * in.fallback.a * (1.0 - tex.a)) / alpha;
    }
    return vec4<f32>(rgb, alpha * edge_alpha * in.opacity);
}
