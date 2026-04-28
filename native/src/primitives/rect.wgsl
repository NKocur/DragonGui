// Instanced rectangle renderer with solid or gradient paint and optional
// rounded corners.
//
// Each instance specifies a pixel-space rect (x, y, w, h), primary/secondary
// RGBA colours, paint metadata, and four corner radii in pixels. The vertex
// shader expands the instance into a two-triangle quad; the fragment shader
// clips pixels outside the rounded corner arc using a signed-distance field.

struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct RectInstance {
    @location(0) rect:  vec4<f32>,  // x, y, w, h (pixels, top-left origin)
    @location(1) color: vec4<f32>,  // rgba linear
    @location(2) radii: vec4<f32>,  // TL, TR, BR, BL corner radii (pixels)
    @location(3) clip_bounds: vec4<f32>, // local left, top, right, bottom clip
    @location(4) params: vec4<f32>, // x softness, y shape inset, z shadow mode, w shape kind
    @location(5) color2: vec4<f32>, // secondary rgba for gradient paints
    @location(6) paint: vec4<f32>, // x paint kind, y/z linear gradient direction, w stop count or shape option
    @location(7) transform: vec4<f32>, // x/y translation pixels, z/w scale
    @location(8) transform2: vec4<f32>, // x rotation radians, y background noise strength
    @location(9) color3: vec4<f32>, // third rgba for gradient paints
    @location(10) color4: vec4<f32>, // fourth rgba for gradient paints
    @location(11) gradient_stops: vec4<f32>, // stop positions for color-color4
}

struct VertOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Pixel coordinates within the rect, origin at the rect's top-left.
    @location(1) local_px: vec2<f32>,
    // Half-size and radii are constant per instance; mark flat to avoid
    // interpolation artifacts at triangle edges.
    @location(2) @interpolate(flat) half_size: vec2<f32>,
    @location(3) @interpolate(flat) radii: vec4<f32>,
    @location(4) @interpolate(flat) clip_bounds: vec4<f32>,
    @location(5) @interpolate(flat) params: vec4<f32>,
    @location(6) @interpolate(flat) color2: vec4<f32>,
    @location(7) @interpolate(flat) paint: vec4<f32>,
    @location(8) @interpolate(flat) color3: vec4<f32>,
    @location(9) @interpolate(flat) color4: vec4<f32>,
    @location(10) @interpolate(flat) gradient_stops: vec4<f32>,
    @location(11) @interpolate(flat) noise: f32,
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
    let aa_pad = 1.0;
    let uv = QUAD[vid];
    let expanded_size = inst.rect.zw + vec2<f32>(aa_pad * 2.0, aa_pad * 2.0);
    let px = inst.rect.xy - vec2<f32>(aa_pad, aa_pad) + uv * expanded_size;
    let center = inst.rect.xy + inst.rect.zw * 0.5;
    let scaled = (px - center) * inst.transform.zw;
    let angle = inst.transform2.x;
    let c = cos(angle);
    let s = sin(angle);
    let rotated = vec2<f32>(
        scaled.x * c - scaled.y * s,
        scaled.x * s + scaled.y * c,
    );
    let transformed_px = center + rotated + inst.transform.xy;
    // pixel -> NDC: x: [0, W] -> [-1, 1]; y flipped: [0, H] -> [1, -1]
    let ndc = vec2<f32>(
        transformed_px.x / u.screen_size.x * 2.0 - 1.0,
        -transformed_px.y / u.screen_size.y * 2.0 + 1.0,
    );
    var out: VertOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.color = inst.color;
    out.local_px = uv * expanded_size - vec2<f32>(aa_pad, aa_pad);
    out.half_size = inst.rect.zw * 0.5;
    out.radii = inst.radii;
    out.clip_bounds = inst.clip_bounds;
    out.params = inst.params;
    out.color2 = inst.color2;
    out.paint = inst.paint;
    out.color3 = inst.color3;
    out.color4 = inst.color4;
    out.gradient_stops = inst.gradient_stops;
    out.noise = inst.transform2.y;
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

// Box-SDF with independently selectable corner radii. Each fragment uses the
// radius for its quadrant. Uniform radii produce the same shape as the previous
// single-radius path.
fn rounded_rect_sdf(p: vec2<f32>, half_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let r = corner_radius(p, half_size, radii);
    let d = abs(p) - half_size + vec2<f32>(r, r);
    return length(max(d, vec2<f32>(0.0, 0.0))) + min(max(d.x, d.y), 0.0) - r;
}

fn cross2(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return a.x * b.y - a.y * b.x;
}

fn segment_distance(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 0.001), 0.0, 1.0);
    return length(pa - ba * h);
}

fn triangle_sdf(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    let d = min(
        segment_distance(p, a, b),
        min(segment_distance(p, b, c), segment_distance(p, c, a)),
    );
    let s1 = cross2(b - a, p - a);
    let s2 = cross2(c - b, p - b);
    let s3 = cross2(a - c, p - c);
    let has_neg = (s1 < 0.0) || (s2 < 0.0) || (s3 < 0.0);
    let has_pos = (s1 > 0.0) || (s2 > 0.0) || (s3 > 0.0);
    let inside = !(has_neg && has_pos);
    return select(d, -d, inside);
}

fn gradient_segment_color(
    t: f32,
    left_t: f32,
    right_t: f32,
    left_color: vec4<f32>,
    right_color: vec4<f32>,
) -> vec4<f32> {
    let span = max(right_t - left_t, 0.0001);
    return mix(left_color, right_color, clamp((t - left_t) / span, 0.0, 1.0));
}

fn gradient_color_at(in: VertOut, t: f32) -> vec4<f32> {
    let raw_count = in.paint.w;
    let count = max(abs(raw_count), 2.0);
    let s0 = clamp(in.gradient_stops.x, 0.0, 1.0);
    let s1 = clamp(max(in.gradient_stops.y, s0), 0.0, 1.0);
    let s2 = clamp(max(in.gradient_stops.z, s1), 0.0, 1.0);
    let s3 = clamp(max(in.gradient_stops.w, s2), 0.0, 1.0);
    var local_t = t;
    if (raw_count < -0.5) {
        var period = s1;
        if (count >= 3.5) {
            period = s3;
        } else if (count >= 2.5) {
            period = s2;
        }
        if (period > 0.0001 && period < 0.9999) {
            local_t = local_t - floor(local_t / period) * period;
        }
    }

    if (count < 2.5) {
        return gradient_segment_color(local_t, s0, s1, in.color, in.color2);
    }
    if (local_t <= s1) {
        return gradient_segment_color(local_t, s0, s1, in.color, in.color2);
    }
    if (count < 3.5) {
        return gradient_segment_color(local_t, s1, s2, in.color2, in.color3);
    }
    if (local_t <= s2) {
        return gradient_segment_color(local_t, s1, s2, in.color2, in.color3);
    }
    return gradient_segment_color(local_t, s2, s3, in.color3, in.color4);
}

fn hash_noise(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
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
    let shape_inset = max(in.params.y, 0.0);
    let shape_half_size = max(in.half_size - vec2<f32>(shape_inset, shape_inset), vec2<f32>(0.5, 0.5));
    let p = in.local_px - in.half_size; // centered coords
    var sdf: f32;
    if (in.params.w > 0.5 && in.params.w < 1.5) {
        let rounding = clamp(in.radii.x, 0.0, min(shape_half_size.x, shape_half_size.y) * 0.45);
        let tri_half = max(shape_half_size - vec2<f32>(rounding, rounding), vec2<f32>(0.5, 0.5));
        var a = vec2<f32>(0.0, 0.0);
        var b = vec2<f32>(0.0, 0.0);
        var c = vec2<f32>(0.0, 0.0);
        if (in.paint.w > 0.5) {
            a = vec2<f32>(-tri_half.x, tri_half.y);
            b = vec2<f32>(tri_half.x, tri_half.y);
            c = vec2<f32>(0.0, -tri_half.y);
        } else {
            a = vec2<f32>(-tri_half.x, -tri_half.y);
            b = vec2<f32>(tri_half.x, -tri_half.y);
            c = vec2<f32>(0.0, tri_half.y);
        }
        sdf = triangle_sdf(p, a, b, c) - rounding;
    } else {
        sdf = rounded_rect_sdf(p, shape_half_size, in.radii);
    }
    // Anti-aliased edge: smoothly fade over 1 pixel.
    var color = in.color;
    if (in.paint.x > 0.5 && in.paint.x < 1.5 && in.params.z < 0.5) {
        let size = max(in.half_size * 2.0, vec2<f32>(1.0, 1.0));
        let uv = in.local_px / size;
        let dir = normalize(in.paint.yz);
        let t = clamp(dot(uv - vec2<f32>(0.5, 0.5), dir) + 0.5, 0.0, 1.0);
        color = gradient_color_at(in, t);
    } else if (in.paint.x > 1.5 && in.paint.x < 2.5 && in.params.z < 0.5) {
        let size = max(in.half_size * 2.0, vec2<f32>(1.0, 1.0));
        let center = clamp(in.paint.yz, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
        let uv = in.local_px / size;
        let aspect = vec2<f32>(
            size.x / max(size.x, size.y),
            size.y / max(size.x, size.y),
        );
        let scaled = (uv - center) / max(aspect, vec2<f32>(0.001, 0.001));
        let c0 = (vec2<f32>(0.0, 0.0) - center) / max(aspect, vec2<f32>(0.001, 0.001));
        let c1 = (vec2<f32>(1.0, 0.0) - center) / max(aspect, vec2<f32>(0.001, 0.001));
        let c2 = (vec2<f32>(0.0, 1.0) - center) / max(aspect, vec2<f32>(0.001, 0.001));
        let c3 = (vec2<f32>(1.0, 1.0) - center) / max(aspect, vec2<f32>(0.001, 0.001));
        let max_dist = max(max(length(c0), length(c1)), max(length(c2), length(c3)));
        let t = clamp(length(scaled) / max(max_dist, 0.001), 0.0, 1.0);
        color = gradient_color_at(in, t);
    }
    if (in.noise > 0.0001 && in.params.z < 0.5) {
        let n = hash_noise(floor(in.local_px + in.clip_bounds.xy * 0.173)) - 0.5;
        color = vec4<f32>(
            clamp(color.rgb + vec3<f32>(n * in.noise), vec3<f32>(0.0), vec3<f32>(1.0)),
            color.a,
        );
    }
    var a: f32;
    if (in.params.z > 0.5) {
        let softness = max(in.params.x, 1.0);
        a = (1.0 - smoothstep(0.0, softness, sdf)) * color.a;
    } else {
        a = clamp(1.0 - sdf, 0.0, 1.0) * color.a;
    }
    if a < 0.001 { discard; }
    return vec4<f32>(color.rgb, a);
}
