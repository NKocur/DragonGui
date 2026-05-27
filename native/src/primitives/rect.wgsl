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
    @location(4) params: vec4<f32>, // x softness, y shape inset, z shadow mode (1 outset, 2 inset), w shape kind
    @location(5) color2: vec4<f32>, // secondary rgba for gradient paints
    @location(6) paint: vec4<f32>, // x paint kind, y/z linear gradient direction, w stop count or shape option
    @location(7) transform: vec4<f32>, // x/y translation pixels, z/w scale
    @location(8) transform2: vec4<f32>, // x rotation, y noise, z gradient interpolation mode
    @location(9) color3: vec4<f32>, // third rgba for gradient paints
    @location(10) color4: vec4<f32>, // fourth rgba for gradient paints
    @location(11) gradient_stops: vec4<f32>, // stop positions for color-color4
    @location(12) color5: vec4<f32>, // fifth rgba for gradient paints
    @location(13) color6: vec4<f32>, // sixth rgba for gradient paints
    @location(14) gradient_stops2: vec4<f32>, // additional gradient stop positions
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
    @location(11) @interpolate(flat) effects: vec2<f32>,
    @location(12) @interpolate(flat) color5: vec4<f32>,
    @location(13) @interpolate(flat) color6: vec4<f32>,
    @location(14) @interpolate(flat) gradient_stops2: vec4<f32>,
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
    out.effects = inst.transform2.yz;
    out.color5 = inst.color5;
    out.color6 = inst.color6;
    out.gradient_stops2 = inst.gradient_stops2;
    return out;
}

fn background_noise_strength(in: VertOut) -> f32 {
    return in.effects.x;
}

fn gradient_interpolation_mode(in: VertOut) -> f32 {
    return in.effects.y;
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
    mode: f32,
) -> vec4<f32> {
    let span = max(right_t - left_t, 0.0001);
    let amount = clamp((t - left_t) / span, 0.0, 1.0);
    let left_alpha = clamp(left_color.a, 0.0, 1.0);
    let right_alpha = clamp(right_color.a, 0.0, 1.0);
    let alpha = mix(left_alpha, right_alpha, amount);
    if (alpha <= 0.0001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let left_space = gradient_color_to_space(left_color.rgb, mode) * left_alpha;
    let right_space = gradient_color_to_space(right_color.rgb, mode) * right_alpha;
    let mixed = mix(left_space, right_space, amount) / alpha;
    return vec4<f32>(gradient_color_from_space(mixed, mode), alpha);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let clamped = max(color, vec3<f32>(0.0));
    let lo = clamped * 12.92;
    let hi = 1.055 * pow(clamped, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return clamp(select(hi, lo, clamped <= vec3<f32>(0.0031308)), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn linear_srgb_to_oklab(c: vec3<f32>) -> vec3<f32> {
    let l = 0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b;
    let m = 0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b;
    let s = 0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b;
    let lms = pow(max(vec3<f32>(l, m, s), vec3<f32>(0.0)), vec3<f32>(1.0 / 3.0));
    return vec3<f32>(
        0.2104542553 * lms.x + 0.7936177850 * lms.y - 0.0040720468 * lms.z,
        1.9779984951 * lms.x - 2.4285922050 * lms.y + 0.4505937099 * lms.z,
        0.0259040371 * lms.x + 0.7827717662 * lms.y - 0.8086757660 * lms.z,
    );
}

fn oklab_to_linear_srgb(c: vec3<f32>) -> vec3<f32> {
    let l_ = c.x + 0.3963377774 * c.y + 0.2158037573 * c.z;
    let m_ = c.x - 0.1055613458 * c.y - 0.0638541728 * c.z;
    let s_ = c.x - 0.0894841775 * c.y - 1.2914855480 * c.z;
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;
    return vec3<f32>(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    );
}

fn gradient_color_to_space(color: vec3<f32>, mode: f32) -> vec3<f32> {
    let clamped_color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    if (mode > 1.5) {
        return linear_srgb_to_oklab(srgb_to_linear(clamped_color));
    }
    if (mode > 0.5) {
        return srgb_to_linear(clamped_color);
    }
    return clamped_color;
}

fn gradient_color_from_space(color: vec3<f32>, mode: f32) -> vec3<f32> {
    if (mode > 1.5) {
        return linear_to_srgb(oklab_to_linear_srgb(color));
    }
    if (mode > 0.5) {
        return linear_to_srgb(color);
    }
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn blob_color_at(in: VertOut) -> vec4<f32> {
    let size = max(in.half_size * 2.0, vec2<f32>(1.0, 1.0));
    let uv = in.local_px / size;
    let aspect = vec2<f32>(
        size.x / max(size.x, size.y),
        size.y / max(size.x, size.y),
    );
    let count = max(in.paint.w, 1.0);
    var field = 0.0;
    var color_sum = vec3<f32>(0.0);
    var alpha_sum = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        if (f32(i) >= count) {
            continue;
        }
        var center = vec2<f32>(0.5, 0.5);
        var radius = 0.42;
        var color = in.color;
        if (i == 0) {
            center = in.gradient_stops.xy;
            radius = in.color5.x;
            color = in.color;
        } else if (i == 1) {
            center = in.gradient_stops.zw;
            radius = in.color5.y;
            color = in.color2;
        } else if (i == 2) {
            center = in.gradient_stops2.xy;
            radius = in.color5.z;
            color = in.color3;
        } else {
            center = in.gradient_stops2.zw;
            radius = in.color5.w;
            color = in.color4;
        }
        let warp = vec2<f32>(
            sin((uv.y + center.x * 1.73) * 10.0 + center.y * 13.0),
            cos((uv.x + center.y * 1.41) * 9.0 + center.x * 11.0),
        ) * 0.025;
        let delta = (uv + warp - center) / max(aspect, vec2<f32>(0.001, 0.001));
        let d2 = dot(delta, delta);
        let r = max(radius, 0.035);
        let influence = exp(-d2 / max(r * r, 0.001));
        let alpha = clamp(color.a, 0.0, 1.0);
        field += influence * alpha;
        color_sum += gradient_color_to_space(color.rgb, gradient_interpolation_mode(in)) * influence * alpha;
        alpha_sum += influence * alpha;
    }
    if (alpha_sum <= 0.0001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let thresholded_alpha = smoothstep(0.18, 0.82, field);
    let color = gradient_color_from_space(color_sum / alpha_sum, gradient_interpolation_mode(in));
    return vec4<f32>(color, thresholded_alpha);
}

fn mesh_color_at(in: VertOut) -> vec4<f32> {
    let size = max(in.half_size * 2.0, vec2<f32>(1.0, 1.0));
    let uv = clamp(in.local_px / size, vec2<f32>(0.0), vec2<f32>(1.0));
    let mode = gradient_interpolation_mode(in);
    let tl = gradient_color_to_space(in.color.rgb, mode) * in.color.a;
    let tr = gradient_color_to_space(in.color2.rgb, mode) * in.color2.a;
    let bl = gradient_color_to_space(in.color3.rgb, mode) * in.color3.a;
    let br = gradient_color_to_space(in.color4.rgb, mode) * in.color4.a;
    let top_alpha = mix(in.color.a, in.color2.a, uv.x);
    let bottom_alpha = mix(in.color3.a, in.color4.a, uv.x);
    let alpha = mix(top_alpha, bottom_alpha, uv.y);
    if (alpha <= 0.0001) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let top = mix(tl, tr, uv.x);
    let bottom = mix(bl, br, uv.x);
    let color = gradient_color_from_space(mix(top, bottom, uv.y) / alpha, mode);
    return vec4<f32>(color, alpha);
}

fn gradient_color_at(in: VertOut, t: f32) -> vec4<f32> {
    let raw_count = in.paint.w;
    let mode = gradient_interpolation_mode(in);
    let count = max(abs(raw_count), 2.0);
    let s0 = clamp(in.gradient_stops.x, 0.0, 1.0);
    let s1 = clamp(max(in.gradient_stops.y, s0), 0.0, 1.0);
    let s2 = clamp(max(in.gradient_stops.z, s1), 0.0, 1.0);
    let s3 = clamp(max(in.gradient_stops.w, s2), 0.0, 1.0);
    let s4 = clamp(max(in.gradient_stops2.x, s3), 0.0, 1.0);
    let s5 = clamp(max(in.gradient_stops2.y, s4), 0.0, 1.0);
    var local_t = t;
    if (raw_count < -0.5) {
        var period = s1;
        if (count >= 5.5) {
            period = s5;
        } else if (count >= 4.5) {
            period = s4;
        } else if (count >= 3.5) {
            period = s3;
        } else if (count >= 2.5) {
            period = s2;
        }
        if (period > 0.0001 && period < 0.9999) {
            local_t = local_t - floor(local_t / period) * period;
        }
    }

    if (count < 2.5) {
        return gradient_segment_color(local_t, s0, s1, in.color, in.color2, mode);
    }
    if (local_t <= s1) {
        return gradient_segment_color(local_t, s0, s1, in.color, in.color2, mode);
    }
    if (count < 3.5) {
        return gradient_segment_color(local_t, s1, s2, in.color2, in.color3, mode);
    }
    if (local_t <= s2) {
        return gradient_segment_color(local_t, s1, s2, in.color2, in.color3, mode);
    }
    if (count < 4.5) {
        return gradient_segment_color(local_t, s2, s3, in.color3, in.color4, mode);
    }
    if (local_t <= s3) {
        return gradient_segment_color(local_t, s2, s3, in.color3, in.color4, mode);
    }
    if (count < 5.5) {
        return gradient_segment_color(local_t, s3, s4, in.color4, in.color5, mode);
    }
    if (local_t <= s4) {
        return gradient_segment_color(local_t, s3, s4, in.color4, in.color5, mode);
    }
    return gradient_segment_color(local_t, s4, s5, in.color5, in.color6, mode);
}

fn hash_noise(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
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
    if (in.params.w > 3.5 && in.params.w < 4.5) {
        let radius = min(in.half_size.x, in.half_size.y);
        let n = p / max(radius, 0.001);
        let dist = length(n);
        if (dist > 1.0) {
            discard;
        }

        let cr = in.gradient_stops2.x;
        let sr = in.gradient_stops2.y;
        let body = vec2<f32>(
            n.x * cr - n.y * sr,
            n.x * sr + n.y * cr,
        );
        let z = sqrt(max(1.0 - dist * dist, 0.0));
        let cp = in.gradient_stops2.z;
        let sp = in.gradient_stops2.w;
        let sphere_p = vec3<f32>(
            body.x,
            body.y * cp - z * sp,
            body.y * sp + z * cp,
        );
        let horizon = sphere_p.y;
        let blend = smoothstep(-0.018, 0.018, horizon);
        var color = mix(in.color, in.color2, blend);

        if (in.paint.x > 1.5) {
            let lat = asin(clamp(sphere_p.y, -1.0, 1.0));
            let lon = atan2(sphere_p.x, sphere_p.z);
            let lat_band = abs(fract((lat + 8.0) / 0.42) - 0.5) * 0.42;
            let lon_band = abs(fract((lon + 8.0) / 0.52) - 0.5) * 0.52;
            let rim_fade = 1.0 - smoothstep(0.86, 0.99, dist);
            let grid = (1.0 - smoothstep(0.008, 0.016, min(lat_band, lon_band))) * rim_fade;
            color = mix(color, in.color3, grid * 0.16);
        } else if (in.paint.x > 0.5) {
            let lat_band = abs(fract((sphere_p.y + 4.0) / 0.22) - 0.5) * 0.22;
            let meridian_a = min(abs(sphere_p.x), abs(sphere_p.z));
            let meridian_b = min(
                abs((sphere_p.x + sphere_p.z) * 0.70710678),
                abs((sphere_p.x - sphere_p.z) * 0.70710678),
            );
            let rim_fade = 1.0 - smoothstep(0.84, 0.98, dist);
            let grid = (1.0 - smoothstep(0.006, 0.014, min(lat_band, min(meridian_a, meridian_b)))) * rim_fade;
            color = mix(color, in.color3, grid * 0.15);
        }

        let horizon_line = 1.0 - smoothstep(0.010, 0.030, abs(horizon));
        color = mix(color, in.color3, horizon_line * 0.36);
        let rim = smoothstep(0.82, 1.0, dist);
        let shade = 0.74 + 0.26 * clamp(1.0 - dist * 0.72 - n.x * 0.10 - n.y * 0.18, 0.0, 1.0);
        color = vec4<f32>(color.rgb * shade, color.a);
        color = mix(color, in.color5, rim * 0.72);
        let alpha = (1.0 - smoothstep(0.985, 1.0, dist)) * color.a;
        if (alpha < 0.001) {
            discard;
        }
        return vec4<f32>(color.rgb, alpha);
    }
    var sdf: f32;
    if (in.params.w > 1.5 && in.params.w < 2.5) {
        let radius = min(shape_half_size.x, shape_half_size.y);
        let dist = length(p);
        let outer_sdf = dist - radius;
        let inner_ratio = clamp(in.paint.w, 0.0, 0.9);
        let inner_sdf = inner_ratio * radius - dist;
        let angle = atan2(p.y, p.x);
        let tau = 6.28318530718;
        let start = in.paint.y;
        let end = in.paint.z;
        let sweep = max(end - start, 0.0001);
        var rel = angle - start;
        rel = rel - floor(rel / tau) * tau;
        let angle_sdf = max(-rel, rel - sweep);
        let angle_px = angle_sdf * radius;
        sdf = max(max(outer_sdf, inner_sdf), angle_px);
    } else if (in.params.w > 0.5 && in.params.w < 1.5) {
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
    } else if (in.paint.x > 2.5 && in.paint.x < 3.5 && in.params.z < 0.5) {
        color = blob_color_at(in);
    } else if (in.paint.x > 3.5 && in.paint.x < 4.5 && in.params.z < 0.5) {
        color = mesh_color_at(in);
    }
    let noise = background_noise_strength(in);
    if (noise > 0.0001 && in.params.z < 0.5) {
        let n = hash_noise(floor(in.local_px + in.clip_bounds.xy * 0.173)) - 0.5;
        color = vec4<f32>(
            clamp(color.rgb + vec3<f32>(n * noise * 0.45), vec3<f32>(0.0), vec3<f32>(1.0)),
            color.a,
        );
    }
    var a: f32;
    if (in.params.z > 2.5 && in.params.z < 3.5) {
        let thickness = max(in.paint.w, 0.0);
        let outer_mask = 1.0 - smoothstep(0.0, 1.0, sdf);
        let inner_half_size = max(shape_half_size - vec2<f32>(thickness, thickness), vec2<f32>(0.5, 0.5));
        let inner_radii = max(in.radii - vec4<f32>(thickness, thickness, thickness, thickness), vec4<f32>(0.0, 0.0, 0.0, 0.0));
        let inner_sdf = rounded_rect_sdf(p, inner_half_size, inner_radii);
        let inner_mask = 1.0 - smoothstep(0.0, 1.0, inner_sdf);
        a = max(outer_mask - inner_mask, 0.0) * color.a;
    } else if (in.params.z > 1.5 && in.params.z < 2.5) {
        let softness = max(in.params.x, 1.0);
        let spread = in.paint.w;
        let inner_half_size = max(shape_half_size - vec2<f32>(spread, spread), vec2<f32>(0.5, 0.5));
        let inner_radii = max(in.radii - vec4<f32>(spread, spread, spread, spread), vec4<f32>(0.0, 0.0, 0.0, 0.0));
        let inner_sdf = rounded_rect_sdf(p - in.paint.yz, inner_half_size, inner_radii);
        let shape_mask = clamp(1.0 - sdf, 0.0, 1.0);
        a = smoothstep(-softness, 0.0, inner_sdf) * shape_mask * color.a;
    } else if (in.params.z > 0.5) {
        let softness = max(in.params.x, 1.0);
        a = (1.0 - smoothstep(0.0, softness, sdf)) * color.a;
    } else {
        a = clamp(1.0 - sdf, 0.0, 1.0) * color.a;
    }
    if a < 0.001 { discard; }
    return vec4<f32>(color.rgb, a);
}
