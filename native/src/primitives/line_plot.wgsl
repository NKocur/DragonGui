// Dedicated LinePlot path.
//
// Points live in a compact storage buffer. Each series is drawn from one
// per-series instance, and the vertex shader expands every segment into a
// screen-space quad with full endpoint disks. Fragment clipping and line-style
// patterns keep plot-edge cuts and dash caps sharp without emitting per-dash
// CPU primitives.

struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct LinePlotPoint {
    data: vec4<f32>,
}

@group(0) @binding(1)
var<storage, read> points: array<LinePlotPoint>;

struct SeriesInstance {
    @location(0) plot: vec4<f32>,
    @location(1) clip_rect: vec4<f32>,
    @location(2) bounds: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) style: vec4<f32>,
}

struct ClipResult {
    a: vec2<f32>,
    b: vec2<f32>,
    visible: f32,
    t0: f32,
}

struct VertOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local: vec2<f32>,
    @location(2) screen_px: vec2<f32>,
    @location(3) @interpolate(flat) segment_len: f32,
    @location(4) @interpolate(flat) radius: f32,
    @location(5) @interpolate(flat) clip_rect: vec4<f32>,
    @location(6) @interpolate(flat) visible: f32,
    @location(7) @interpolate(flat) aa_width: f32,
    @location(8) @interpolate(flat) style_code: f32,
    @location(9) @interpolate(flat) style_start: f32,
    @location(10) @interpolate(flat) line_width: f32,
}

const QUAD: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, -1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(0.0, 1.0),
);

fn map_point(point: vec2<f32>, plot: vec4<f32>, bounds: vec4<f32>) -> vec2<f32> {
    let x_span = max(bounds.y - bounds.x, 0.000001);
    let y_span = max(bounds.w - bounds.z, 0.000001);
    let tx = (point.x - bounds.x) / x_span;
    let ty = (point.y - bounds.z) / y_span;
    return vec2<f32>(plot.x + plot.z * tx, plot.y + plot.w * (1.0 - ty));
}

fn clip_segment_to_rect(a_in: vec2<f32>, b_in: vec2<f32>, rect: vec4<f32>) -> ClipResult {
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.z;
    let bottom = rect.y + rect.w;
    let d = b_in - a_in;
    var t0 = 0.0;
    var t1 = 1.0;
    var visible = true;

    var p = -d.x;
    var q = a_in.x - left;
    if (abs(p) <= 0.000001) {
        visible = visible && q >= 0.0;
    } else {
        let r = q / p;
        if (p < 0.0) {
            if (r > t1) {
                visible = false;
            }
            t0 = max(t0, r);
        } else {
            if (r < t0) {
                visible = false;
            }
            t1 = min(t1, r);
        }
    }

    p = d.x;
    q = right - a_in.x;
    if (abs(p) <= 0.000001) {
        visible = visible && q >= 0.0;
    } else {
        let r = q / p;
        if (p < 0.0) {
            if (r > t1) {
                visible = false;
            }
            t0 = max(t0, r);
        } else {
            if (r < t0) {
                visible = false;
            }
            t1 = min(t1, r);
        }
    }

    p = -d.y;
    q = a_in.y - top;
    if (abs(p) <= 0.000001) {
        visible = visible && q >= 0.0;
    } else {
        let r = q / p;
        if (p < 0.0) {
            if (r > t1) {
                visible = false;
            }
            t0 = max(t0, r);
        } else {
            if (r < t0) {
                visible = false;
            }
            t1 = min(t1, r);
        }
    }

    p = d.y;
    q = bottom - a_in.y;
    if (abs(p) <= 0.000001) {
        visible = visible && q >= 0.0;
    } else {
        let r = q / p;
        if (p < 0.0) {
            if (r > t1) {
                visible = false;
            }
            t0 = max(t0, r);
        } else {
            if (r < t0) {
                visible = false;
            }
            t1 = min(t1, r);
        }
    }

    visible = visible && t0 <= t1;
    let a = a_in + d * t0;
    let b = a_in + d * t1;
    return ClipResult(a, b, select(0.0, 1.0, visible), t0);
}

fn interval_sdf(
    local: vec2<f32>,
    global_start: f32,
    global_end: f32,
    style_start: f32,
    segment_len: f32,
    radius: f32,
) -> f32 {
    let start = clamp(global_start - style_start, 0.0, segment_len);
    let end = clamp(global_end - style_start, 0.0, segment_len);
    if (end <= start) {
        return 100000.0;
    }
    let nearest_x = clamp(local.x, start, end);
    return length(vec2<f32>(local.x - nearest_x, local.y)) - radius;
}

fn patterned_sdf(
    local: vec2<f32>,
    style_x: f32,
    style_start: f32,
    segment_len: f32,
    radius: f32,
    line_width: f32,
    style_code: f32,
) -> f32 {
    if (style_code < 0.5) {
        let nearest_x = clamp(local.x, 0.0, segment_len);
        return length(vec2<f32>(local.x - nearest_x, local.y)) - radius;
    }

    let unit = max(line_width, 1.0);
    var cycle = 14.0 * unit;
    var on0 = 9.0 * unit;
    var gap0 = 5.0 * unit;
    var on1 = 0.0;

    if (style_code >= 1.5 && style_code < 2.5) {
        on0 = max(1.2 * unit, unit);
        gap0 = 4.0 * unit;
        cycle = on0 + gap0;
    } else if (style_code >= 2.5) {
        on0 = 8.0 * unit;
        gap0 = 4.0 * unit;
        on1 = max(1.4 * unit, unit);
        cycle = on0 + gap0 + on1 + 4.0 * unit;
    }

    let base = floor(style_x / cycle) * cycle;
    var sdf = 100000.0;
    for (var i = -1; i <= 1; i = i + 1) {
        let cycle_start = base + f32(i) * cycle;
        sdf = min(
            sdf,
            interval_sdf(local, cycle_start, cycle_start + on0, style_start, segment_len, radius),
        );
        if (style_code >= 2.5) {
            let dot_start = cycle_start + on0 + gap0;
            sdf = min(
                sdf,
                interval_sdf(local, dot_start, dot_start + on1, style_start, segment_len, radius),
            );
        }
    }
    return sdf;
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: SeriesInstance) -> VertOut {
    let segment = vid / 6u;
    let corner = QUAD[vid % 6u];
    let point_offset = u32(inst.params.y + 0.5);
    let p0_data = points[point_offset + segment].data;
    let p1_data = points[point_offset + segment + 1u].data;
    let a_screen = map_point(p0_data.xy, inst.plot, inst.bounds);
    let b_screen = map_point(p1_data.xy, inst.plot, inst.bounds);
    let clipped = clip_segment_to_rect(a_screen, b_screen, inst.clip_rect);

    let line_width = max(inst.params.x, 1.0);
    let radius = max(line_width * 0.5, 0.5);
    let aa = max(inst.params.w, 0.5);
    var screen_px = vec2<f32>(-100000.0, -100000.0);
    var local = vec2<f32>(0.0, 0.0);
    var segment_len = 1.0;
    var style_start = 0.0;

    if (clipped.visible > 0.5) {
        let original_len = max(length(b_screen - a_screen), 0.001);
        let delta = clipped.b - clipped.a;
        segment_len = max(length(delta), 0.001);
        style_start = p0_data.z + original_len * clipped.t0;
        let dir = delta / segment_len;
        let normal = vec2<f32>(-dir.y, dir.x);
        let local_x = mix(-radius - aa, segment_len + radius + aa, corner.x);
        let local_y = corner.y * (radius + aa);
        screen_px = clipped.a + dir * local_x + normal * local_y;
        local = vec2<f32>(local_x, local_y);
    }

    let ndc = vec2<f32>(
        screen_px.x / u.screen_size.x * 2.0 - 1.0,
        -screen_px.y / u.screen_size.y * 2.0 + 1.0,
    );

    var out: VertOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = inst.color;
    out.local = local;
    out.screen_px = screen_px;
    out.segment_len = segment_len;
    out.radius = radius;
    out.clip_rect = inst.clip_rect;
    out.visible = clipped.visible;
    out.aa_width = aa;
    out.style_code = inst.style.x;
    out.style_start = style_start;
    out.line_width = line_width;
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    if (in.visible < 0.5) {
        discard;
    }
    if (
        in.screen_px.x < in.clip_rect.x ||
        in.screen_px.x > in.clip_rect.x + in.clip_rect.z ||
        in.screen_px.y < in.clip_rect.y ||
        in.screen_px.y > in.clip_rect.y + in.clip_rect.w
    ) {
        discard;
    }
    let style_x = in.style_start + in.local.x;
    let sdf = patterned_sdf(
        in.local,
        style_x,
        in.style_start,
        in.segment_len,
        in.radius,
        in.line_width,
        in.style_code,
    );
    let a = clamp(0.5 - sdf / in.aa_width, 0.0, 1.0) * in.color.a;
    if (a < 0.001) {
        discard;
    }
    return vec4<f32>(in.color.rgb, a);
}
