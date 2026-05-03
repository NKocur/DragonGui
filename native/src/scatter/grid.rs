/// Grid geometry for Scatter3D: bounding box, tick marks, grid planes, and
/// screen-space label anchors. Ported from DragonSci src/grid.rs.
use glam::Vec3;

/// Expand raw data bounds to the nearest "nice" round numbers so the grid
/// stays visually stable between frames that share a similar data range.
pub fn nice_bounds(min: Vec3, max: Vec3) -> (Vec3, Vec3) {
    let nice_axis = |lo: f32, hi: f32| -> (f32, f32) {
        let range = (hi - lo).abs();
        if range < 1e-10 {
            return (lo - 0.5, hi + 0.5);
        }
        let rough_step = range / 5.0;
        let mag = 10_f32.powf(rough_step.log10().floor());
        let norm = rough_step / mag;
        let nice_step = if norm <= 1.0 {
            1.0
        } else if norm <= 2.0 {
            2.0
        } else if norm <= 5.0 {
            5.0
        } else {
            10.0
        } * mag;
        let nice_min = (lo / nice_step).floor() * nice_step;
        let nice_max = (hi / nice_step).ceil() * nice_step;
        (nice_min, nice_max)
    };
    let (x0, x1) = nice_axis(min.x, max.x);
    let (y0, y1) = nice_axis(min.y, max.y);
    let (z0, z1) = nice_axis(min.z, max.z);
    (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1))
}

/// Keep an existing nice grid range while incoming data remains within it.
///
/// This preserves automatic bounds for streaming data without rebuilding the
/// grid on every small frame-to-frame min/max change. Bounds still expand as
/// soon as new data exceeds the current grid, and shrink only when the new
/// nice range is much tighter on every axis.
pub fn sticky_nice_bounds(
    previous: Option<(Vec3, Vec3)>,
    data_min: Vec3,
    data_max: Vec3,
) -> (Vec3, Vec3) {
    let candidate = nice_bounds(data_min, data_max);
    let Some((current_min, current_max)) = previous else {
        return candidate;
    };

    let sticky_axis = |current_lo: f32,
                       current_hi: f32,
                       candidate_lo: f32,
                       candidate_hi: f32,
                       data_lo: f32,
                       data_hi: f32|
     -> (f32, f32) {
        if data_lo < current_lo || data_hi > current_hi {
            return (candidate_lo, candidate_hi);
        }
        let current_span = (current_hi - current_lo).abs().max(1e-6);
        let candidate_span = (candidate_hi - candidate_lo).abs();
        if candidate_span < current_span * 0.55 {
            (candidate_lo, candidate_hi)
        } else {
            (current_lo, current_hi)
        }
    };
    let (x0, x1) = sticky_axis(
        current_min.x,
        current_max.x,
        candidate.0.x,
        candidate.1.x,
        data_min.x,
        data_max.x,
    );
    let (y0, y1) = sticky_axis(
        current_min.y,
        current_max.y,
        candidate.0.y,
        candidate.1.y,
        data_min.y,
        data_max.y,
    );
    let (z0, z1) = sticky_axis(
        current_min.z,
        current_max.z,
        candidate.0.z,
        candidate.1.z,
        data_min.z,
        data_max.z,
    );
    (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1))
}

/// Return the nice tick step for a given range.
pub fn tick_step(lo: f32, hi: f32, max_ticks: usize) -> f32 {
    let range = hi - lo;
    if range < 1e-10 || max_ticks == 0 {
        return 1.0;
    }
    let rough = range / max_ticks as f32;
    let mag = 10_f32.powf(rough.log10().floor());
    let norm = rough / mag;
    let mut step = (if norm <= 1.0 {
        1.0
    } else if norm <= 2.0 {
        2.0
    } else if norm <= 5.0 {
        5.0
    } else {
        10.0
    }) * mag;
    loop {
        let first = (lo / step).ceil() * step;
        let mut count = 0usize;
        let mut t = first;
        while t <= hi + step * 1e-4 {
            count += 1;
            t += step;
        }
        if count <= max_ticks {
            return step;
        }
        let m = 10_f32.powf(step.log10().floor());
        let n = (step / m).round() as i32;
        step = match n {
            1 => 2.0,
            2 => 5.0,
            _ => 10.0,
        } * m;
    }
}

/// Generate tick positions at multiples of the nice step for [lo, hi].
pub fn axis_ticks(lo: f32, hi: f32, max_ticks: usize) -> Vec<f32> {
    let range = hi - lo;
    if range < 1e-10 || max_ticks == 0 {
        return vec![];
    }
    let mut step = tick_step(lo, hi, max_ticks);
    loop {
        let first = (lo / step).ceil() * step;
        let mut ticks = Vec::with_capacity(max_ticks + 1);
        let mut t = first;
        while t <= hi + step * 1e-4 {
            ticks.push(t);
            t += step;
        }
        if ticks.len() <= max_ticks {
            return ticks;
        }
        let mag = 10_f32.powf(step.log10().floor());
        let norm = (step / mag).round() as i32;
        step = match norm {
            1 => 2.0,
            2 => 5.0,
            _ => 10.0,
        } * mag;
    }
}

fn minor_ticks(lo: f32, hi: f32, major_step: f32, subdivisions: u32) -> Vec<f32> {
    if major_step <= 0.0 || subdivisions <= 1 {
        return vec![];
    }
    let minor_step = major_step / subdivisions as f32;
    let first = (lo / minor_step).ceil() * minor_step;
    let mut result = Vec::new();
    let mut t = first;
    while t <= hi + minor_step * 1e-4 {
        let dist = ((t / major_step).round() * major_step - t).abs();
        if dist > major_step * 1e-4 {
            result.push(t);
        }
        t += minor_step;
    }
    result
}

/// A line segment vertex: position + RGB color. Pod for GPU upload.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

pub struct LabelAnchor {
    pub world_pos: Vec3,
    pub tick_pos: Vec3,
    pub text: String,
    pub is_axis_title: bool,
}

pub struct GridGeometry {
    pub vertices: Vec<LineVertex>,
    pub labels: Vec<LabelAnchor>,
}

/// Format a tick value for display.
pub fn format_tick(v: f32) -> String {
    if v.abs() >= 1000.0 || (v.abs() < 0.01 && v != 0.0) {
        format!("{:.2e}", v)
    } else {
        format!("{:.3}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// 6-bit face sentinel: identifies which box face the camera is on and whether
/// any axis is a depth axis. Rebuilt grid geometry when this changes.
pub fn face_bits(camera_eye: Vec3, center: Vec3) -> u8 {
    let side = ((camera_eye.y < center.y) as u8)
        | (((camera_eye.z < center.z) as u8) << 1)
        | (((camera_eye.x < center.x) as u8) << 2);
    let cam_dir = (center - camera_eye).normalize_or_zero();
    let depth = ((cam_dir.x.abs() > 0.97) as u8) << 3
        | ((cam_dir.y.abs() > 0.97) as u8) << 4
        | ((cam_dir.z.abs() > 0.97) as u8) << 5;
    side | depth
}

/// Camera-side bits with a small deadband around axis planes.
///
/// Without this, grid labels and planes can flip as soon as the camera crosses
/// the exact center plane. The previous bits are retained while the camera is
/// close to a transition, which keeps interactive rotation visually calmer.
pub fn stable_face_bits(camera_eye: Vec3, center: Vec3, extent: Vec3, previous: u8) -> u8 {
    let current = face_bits(camera_eye, center);
    if previous == 0xFF {
        return current;
    }

    let side_deadband = extent.abs().max(Vec3::splat(1.0)) * 0.04;
    let mut bits = 0_u8;
    let side_bit = |coord: f32, center: f32, deadband: f32, previous_set: bool| -> bool {
        if coord < center - deadband {
            true
        } else if coord > center + deadband {
            false
        } else {
            previous_set
        }
    };
    if side_bit(
        camera_eye.y,
        center.y,
        side_deadband.y,
        previous & 0b000001 != 0,
    ) {
        bits |= 0b000001;
    }
    if side_bit(
        camera_eye.z,
        center.z,
        side_deadband.z,
        previous & 0b000010 != 0,
    ) {
        bits |= 0b000010;
    }
    if side_bit(
        camera_eye.x,
        center.x,
        side_deadband.x,
        previous & 0b000100 != 0,
    ) {
        bits |= 0b000100;
    }

    let cam_dir = (center - camera_eye).normalize_or_zero();
    let depth_bit = |value: f32, bit: u8| -> bool {
        let abs = value.abs();
        if abs > 0.985 {
            true
        } else if abs < 0.94 {
            false
        } else {
            previous & bit != 0
        }
    };
    if depth_bit(cam_dir.x, 0b001000) {
        bits |= 0b001000;
    }
    if depth_bit(cam_dir.y, 0b010000) {
        bits |= 0b010000;
    }
    if depth_bit(cam_dir.z, 0b100000) {
        bits |= 0b100000;
    }
    bits
}

/// Build the bounding-box wireframe, tick marks, grid planes, and label anchors.
pub fn build_grid(
    data_min: Vec3,
    data_max: Vec3,
    nice_min: Vec3,
    nice_max: Vec3,
    tick_override: [Option<usize>; 3],
    axis_visible: [bool; 3],
    camera_eye: Vec3,
    axis_texts: &[String; 3],
    show_major_planes: bool,
    show_minor_planes: bool,
    show_all_edges: bool,
    ortho_scale: Option<(f32, f32)>,
) -> GridGeometry {
    let mut verts: Vec<LineVertex> = Vec::new();
    let mut labels: Vec<LabelAnchor> = Vec::new();

    let extent = nice_max - nice_min;
    let box_color = [0.45_f32, 0.45, 0.45];
    let x_col = [0.90_f32, 0.30, 0.30];
    let y_col = [0.30_f32, 0.90, 0.30];
    let z_col = [0.30_f32, 0.50, 0.90];
    let center = (nice_min + nice_max) * 0.5;

    // ── Bounding box ─────────────────────────────────────────────────────────
    let c = [
        Vec3::new(nice_min.x, nice_min.y, nice_min.z),
        Vec3::new(nice_max.x, nice_min.y, nice_min.z),
        Vec3::new(nice_min.x, nice_max.y, nice_min.z),
        Vec3::new(nice_max.x, nice_max.y, nice_min.z),
        Vec3::new(nice_min.x, nice_min.y, nice_max.z),
        Vec3::new(nice_max.x, nice_min.y, nice_max.z),
        Vec3::new(nice_min.x, nice_max.y, nice_max.z),
        Vec3::new(nice_max.x, nice_max.y, nice_max.z),
    ];
    let edges: [(usize, usize); 12] = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let far_idx: usize = (camera_eye.x >= center.x) as usize
        | ((camera_eye.y >= center.y) as usize) << 1
        | ((camera_eye.z >= center.z) as usize) << 2;

    let mut push_line = |a: Vec3, b: Vec3, color: [f32; 3]| {
        verts.push(LineVertex {
            position: a.to_array(),
            color,
        });
        verts.push(LineVertex {
            position: b.to_array(),
            color,
        });
    };

    if show_all_edges {
        let all_edge_color = [0.28_f32, 0.28, 0.32];
        for (a, b) in &edges {
            push_line(c[*a], c[*b], all_edge_color);
        }
    }

    for (i, (a, b)) in edges.iter().enumerate() {
        if !axis_visible[2] {
            match i {
                0 | 1 | 4 | 5 => {}
                _ => continue,
            }
        } else if *a == far_idx || *b == far_idx {
            continue;
        }
        push_line(c[*a], c[*b], box_color);
    }

    // ── Flat-axis detection ───────────────────────────────────────────────────
    let data_range = data_max - data_min;
    let diagonal = data_range.length().max(1e-10);
    let flat_x = data_range.x.abs() / diagonal < 0.01;
    let flat_y = data_range.y.abs() / diagonal < 0.01;
    let flat_z = data_range.z.abs() / diagonal < 0.01;

    // ── Tick sizing ───────────────────────────────────────────────────────────
    let (tick_len_y_dir, tick_len_x_dir) = if let Some((hw, hh)) = ortho_scale {
        (hh * 2.0 * 0.025, hw * 2.0 * 0.025)
    } else {
        let tl = extent.length() * 0.025;
        (tl, tl)
    };
    let tick_len = tick_len_y_dir;
    let label_offset = tick_len * 2.0;
    let pad = extent.length() * 0.12;

    let max_ne = extent.x.max(extent.y).max(extent.z).max(1e-10);
    let ticks_for = |e: f32| -> usize {
        let r = e / max_ne;
        if r < 0.15 {
            2
        } else if r < 0.40 {
            3
        } else {
            5
        }
    };
    let (x_ticks_default, y_ticks_default) = if ortho_scale.is_some() {
        (5_usize, 5_usize)
    } else {
        (ticks_for(extent.x), ticks_for(extent.y))
    };
    let x_ticks = tick_override[0].unwrap_or(x_ticks_default);
    let y_ticks = tick_override[1].unwrap_or(y_ticks_default);
    let z_ticks = tick_override[2].unwrap_or_else(|| ticks_for(extent.z));

    // ── Dynamic face selection ────────────────────────────────────────────────
    let (
        x_y_edge,
        x_y_sign,
        x_z_edge,
        z_wall_edge,
        y_x_edge,
        y_x_sign,
        y_z_edge,
        z_x_edge,
        z_x_sign,
        z_y_edge,
    ): (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32) = if !axis_visible[2] {
        (
            nice_min.y, -1.0, nice_min.z, nice_min.z, nice_min.x, -1.0, nice_min.z, nice_max.x,
            1.0, nice_min.y,
        )
    } else {
        let (x_y_edge, x_y_sign): (f32, f32) = if camera_eye.y >= center.y {
            (nice_min.y, -1.0)
        } else {
            (nice_max.y, 1.0)
        };
        let x_z_edge: f32 = if camera_eye.z >= center.z {
            nice_max.z
        } else {
            nice_min.z
        };
        let z_wall_edge: f32 = if camera_eye.z >= center.z {
            nice_min.z
        } else {
            nice_max.z
        };
        let (y_x_edge, y_x_sign): (f32, f32) = if camera_eye.x >= center.x {
            (nice_min.x, -1.0)
        } else {
            (nice_max.x, 1.0)
        };
        let y_z_edge: f32 = if camera_eye.z >= center.z {
            nice_max.z
        } else {
            nice_min.z
        };
        let (z_x_edge, z_x_sign): (f32, f32) = if camera_eye.x >= center.x {
            (nice_max.x, 1.0)
        } else {
            (nice_min.x, -1.0)
        };
        let z_y_edge = x_y_edge;
        (
            x_y_edge,
            x_y_sign,
            x_z_edge,
            z_wall_edge,
            y_x_edge,
            y_x_sign,
            y_z_edge,
            z_x_edge,
            z_x_sign,
            z_y_edge,
        )
    };

    // ── Depth-axis detection ──────────────────────────────────────────────────
    let cam_dir = (center - camera_eye).normalize_or_zero();
    let depth_x = cam_dir.x.abs() > 0.97;
    let depth_y = cam_dir.y.abs() > 0.97;
    let depth_z = cam_dir.z.abs() > 0.97;
    let z_out: f32 = if camera_eye.z >= center.z { 1.0 } else { -1.0 };

    let suppress_flat = axis_visible[2];
    let x_show =
        axis_visible[0] && !depth_x && (!suppress_flat || !flat_x || tick_override[0].is_some());
    let y_show =
        axis_visible[1] && !depth_y && (!suppress_flat || !flat_y || tick_override[1].is_some());
    let z_show = axis_visible[2] && !depth_z && (!flat_z || tick_override[2].is_some());

    let x_vals = if x_show {
        axis_ticks(nice_min.x, nice_max.x, x_ticks)
    } else {
        vec![]
    };
    let y_vals = if y_show {
        axis_ticks(nice_min.y, nice_max.y, y_ticks)
    } else {
        vec![]
    };
    let z_vals = if z_show {
        axis_ticks(nice_min.z, nice_max.z, z_ticks)
    } else {
        vec![]
    };

    // ── Grid planes ───────────────────────────────────────────────────────────
    if show_major_planes || show_minor_planes {
        let major_col = [0.20_f32, 0.20, 0.25];
        let minor_col = [0.13_f32, 0.13, 0.17];

        let x_step = if x_vals.len() >= 2 {
            x_vals[1] - x_vals[0]
        } else {
            0.0
        };
        let y_step = if y_vals.len() >= 2 {
            y_vals[1] - y_vals[0]
        } else {
            0.0
        };
        let z_step = if z_vals.len() >= 2 {
            z_vals[1] - z_vals[0]
        } else {
            0.0
        };

        let x_minor = if show_minor_planes {
            minor_ticks(nice_min.x, nice_max.x, x_step, 5)
        } else {
            vec![]
        };
        let y_minor = if show_minor_planes {
            minor_ticks(nice_min.y, nice_max.y, y_step, 5)
        } else {
            vec![]
        };
        let z_minor = if show_minor_planes {
            minor_ticks(nice_min.z, nice_max.z, z_step, 5)
        } else {
            vec![]
        };

        let mut seg = |a: Vec3, b: Vec3, col: [f32; 3]| {
            verts.push(LineVertex {
                position: a.to_array(),
                color: col,
            });
            verts.push(LineVertex {
                position: b.to_array(),
                color: col,
            });
        };

        if !axis_visible[2] {
            let z = nice_min.z;
            if show_major_planes {
                for &x in &x_vals {
                    seg(
                        Vec3::new(x, nice_min.y, z),
                        Vec3::new(x, nice_max.y, z),
                        major_col,
                    );
                }
                for &y in &y_vals {
                    seg(
                        Vec3::new(nice_min.x, y, z),
                        Vec3::new(nice_max.x, y, z),
                        major_col,
                    );
                }
            }
            if show_minor_planes {
                for &x in &x_minor {
                    seg(
                        Vec3::new(x, nice_min.y, z),
                        Vec3::new(x, nice_max.y, z),
                        minor_col,
                    );
                }
                for &y in &y_minor {
                    seg(
                        Vec3::new(nice_min.x, y, z),
                        Vec3::new(nice_max.x, y, z),
                        minor_col,
                    );
                }
            }
        } else {
            if x_show || z_show {
                let y = x_y_edge;
                if show_major_planes {
                    for &x in &x_vals {
                        seg(
                            Vec3::new(x, y, nice_min.z),
                            Vec3::new(x, y, nice_max.z),
                            major_col,
                        );
                    }
                    for &z in &z_vals {
                        seg(
                            Vec3::new(nice_min.x, y, z),
                            Vec3::new(nice_max.x, y, z),
                            major_col,
                        );
                    }
                }
                if show_minor_planes {
                    for &x in &x_minor {
                        seg(
                            Vec3::new(x, y, nice_min.z),
                            Vec3::new(x, y, nice_max.z),
                            minor_col,
                        );
                    }
                    for &z in &z_minor {
                        seg(
                            Vec3::new(nice_min.x, y, z),
                            Vec3::new(nice_max.x, y, z),
                            minor_col,
                        );
                    }
                }
            }
            if y_show || z_show {
                let x = y_x_edge;
                if show_major_planes {
                    for &y in &y_vals {
                        seg(
                            Vec3::new(x, y, nice_min.z),
                            Vec3::new(x, y, nice_max.z),
                            major_col,
                        );
                    }
                    for &z in &z_vals {
                        seg(
                            Vec3::new(x, nice_min.y, z),
                            Vec3::new(x, nice_max.y, z),
                            major_col,
                        );
                    }
                }
                if show_minor_planes {
                    for &y in &y_minor {
                        seg(
                            Vec3::new(x, y, nice_min.z),
                            Vec3::new(x, y, nice_max.z),
                            minor_col,
                        );
                    }
                    for &z in &z_minor {
                        seg(
                            Vec3::new(x, nice_min.y, z),
                            Vec3::new(x, nice_max.y, z),
                            minor_col,
                        );
                    }
                }
            }
            if x_show || y_show {
                let z = z_wall_edge;
                if show_major_planes {
                    for &x in &x_vals {
                        seg(
                            Vec3::new(x, nice_min.y, z),
                            Vec3::new(x, nice_max.y, z),
                            major_col,
                        );
                    }
                    for &y in &y_vals {
                        seg(
                            Vec3::new(nice_min.x, y, z),
                            Vec3::new(nice_max.x, y, z),
                            major_col,
                        );
                    }
                }
                if show_minor_planes {
                    for &x in &x_minor {
                        seg(
                            Vec3::new(x, nice_min.y, z),
                            Vec3::new(x, nice_max.y, z),
                            minor_col,
                        );
                    }
                    for &y in &y_minor {
                        seg(
                            Vec3::new(nice_min.x, y, z),
                            Vec3::new(nice_max.x, y, z),
                            minor_col,
                        );
                    }
                }
            }
        }
    }

    // ── X ticks ───────────────────────────────────────────────────────────────
    if x_show {
        let x_label_off_y = tick_len_y_dir * 2.0;
        let x_pad_y = tick_len_y_dir * 4.8;
        let (x_tick_off, x_label_off, x_pad_off) = if !depth_y {
            (
                Vec3::new(0.0, x_y_sign * tick_len_y_dir, 0.0),
                Vec3::new(0.0, x_y_sign * x_label_off_y, 0.0),
                Vec3::new(0.0, x_y_sign * x_pad_y, 0.0),
            )
        } else {
            (
                Vec3::new(0.0, 0.0, z_out * tick_len),
                Vec3::new(0.0, 0.0, z_out * label_offset),
                Vec3::new(0.0, 0.0, z_out * pad),
            )
        };
        for &val in &x_vals {
            let v = Vec3::new(val, x_y_edge, x_z_edge);
            let end = v + x_tick_off;
            verts.push(LineVertex {
                position: v.to_array(),
                color: x_col,
            });
            verts.push(LineVertex {
                position: end.to_array(),
                color: x_col,
            });
            labels.push(LabelAnchor {
                world_pos: end + x_label_off,
                tick_pos: end,
                text: format_tick(val),
                is_axis_title: false,
            });
        }
        if !axis_texts[0].is_empty() {
            let mid = Vec3::new(center.x, x_y_edge, x_z_edge);
            labels.push(LabelAnchor {
                world_pos: mid + x_pad_off,
                tick_pos: mid,
                text: axis_texts[0].clone(),
                is_axis_title: true,
            });
        }
    }

    // ── Y ticks ───────────────────────────────────────────────────────────────
    if y_show {
        let y_label_off_x = tick_len_x_dir * 2.0;
        let y_pad_x = tick_len_x_dir * 4.8;
        let (y_tick_off, y_label_off, y_pad_off) = if !depth_x {
            (
                Vec3::new(y_x_sign * tick_len_x_dir, 0.0, 0.0),
                Vec3::new(y_x_sign * y_label_off_x, 0.0, 0.0),
                Vec3::new(y_x_sign * y_pad_x, 0.0, 0.0),
            )
        } else {
            (
                Vec3::new(0.0, 0.0, z_out * tick_len),
                Vec3::new(0.0, 0.0, z_out * label_offset),
                Vec3::new(0.0, 0.0, z_out * pad),
            )
        };
        for &val in &y_vals {
            let v = Vec3::new(y_x_edge, val, y_z_edge);
            let end = v + y_tick_off;
            verts.push(LineVertex {
                position: v.to_array(),
                color: y_col,
            });
            verts.push(LineVertex {
                position: end.to_array(),
                color: y_col,
            });
            labels.push(LabelAnchor {
                world_pos: end + y_label_off,
                tick_pos: end,
                text: format_tick(val),
                is_axis_title: false,
            });
        }
        if !axis_texts[1].is_empty() {
            let mid = Vec3::new(y_x_edge, center.y, y_z_edge);
            labels.push(LabelAnchor {
                world_pos: mid + y_pad_off,
                tick_pos: mid,
                text: axis_texts[1].clone(),
                is_axis_title: true,
            });
        }
    }

    // ── Z ticks ───────────────────────────────────────────────────────────────
    if z_show {
        let (z_tick_off, z_label_off, z_pad_off) = if !depth_x {
            (
                Vec3::new(z_x_sign * tick_len, 0.0, 0.0),
                Vec3::new(z_x_sign * label_offset, 0.0, 0.0),
                Vec3::new(z_x_sign * pad, 0.0, 0.0),
            )
        } else {
            (
                Vec3::new(0.0, x_y_sign * tick_len, 0.0),
                Vec3::new(0.0, x_y_sign * label_offset, 0.0),
                Vec3::new(0.0, x_y_sign * pad, 0.0),
            )
        };
        for &val in &z_vals {
            let v = Vec3::new(z_x_edge, z_y_edge, val);
            let end = v + z_tick_off;
            verts.push(LineVertex {
                position: v.to_array(),
                color: z_col,
            });
            verts.push(LineVertex {
                position: end.to_array(),
                color: z_col,
            });
            labels.push(LabelAnchor {
                world_pos: end + z_label_off,
                tick_pos: end,
                text: format_tick(val),
                is_axis_title: false,
            });
        }
        if !axis_texts[2].is_empty() {
            let mid = Vec3::new(z_x_edge, z_y_edge, center.z);
            labels.push(LabelAnchor {
                world_pos: mid + z_pad_off,
                tick_pos: mid,
                text: axis_texts[2].clone(),
                is_axis_title: true,
            });
        }
    }

    GridGeometry {
        vertices: verts,
        labels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_bounds_expands_unit_range() {
        let (lo, hi) = nice_bounds(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(lo.x <= 0.0 && hi.x >= 1.0);
    }

    #[test]
    fn axis_ticks_returns_empty_for_degenerate() {
        assert!(axis_ticks(1.0, 1.0, 5).is_empty());
    }

    #[test]
    fn axis_ticks_respects_max_count() {
        let ticks = axis_ticks(0.0, 100.0, 5);
        assert!(ticks.len() <= 5);
    }

    #[test]
    fn tick_step_matches_axis_ticks() {
        let step = tick_step(0.0, 100.0, 5);
        let ticks = axis_ticks(0.0, 100.0, 5);
        if ticks.len() >= 2 {
            let actual_step = ticks[1] - ticks[0];
            assert!((actual_step - step).abs() < step * 1e-4);
        }
    }

    #[test]
    fn sticky_nice_bounds_keeps_current_range_until_data_exceeds_it() {
        let previous = Some((Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0)));
        let held = sticky_nice_bounds(previous, Vec3::new(1.0, 2.0, 1.5), Vec3::new(8.0, 8.5, 9.0));
        assert_eq!(held, previous.unwrap());

        let expanded = sticky_nice_bounds(
            previous,
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(8.0, 9.0, 9.0),
        );
        assert!(expanded.0.x <= -1.0);
    }

    #[test]
    fn stable_face_bits_keeps_previous_side_inside_deadband() {
        let center = Vec3::ZERO;
        let extent = Vec3::splat(10.0);
        let previous = 0b000001;
        let bits = stable_face_bits(Vec3::new(0.0, 0.1, 4.0), center, extent, previous);
        assert_eq!(bits & 0b000001, previous & 0b000001);

        let crossed = stable_face_bits(Vec3::new(0.0, 1.0, 4.0), center, extent, previous);
        assert_eq!(crossed & 0b000001, 0);
    }

    #[test]
    fn format_tick_trims_trailing_zeros() {
        assert_eq!(format_tick(1.0), "1");
        assert_eq!(format_tick(1.5), "1.5");
    }

    #[test]
    fn format_tick_uses_scientific_for_large() {
        let s = format_tick(12345.0);
        assert!(s.contains('e'), "expected scientific notation, got {s}");
    }

    #[test]
    fn build_grid_produces_vertices_for_unit_box() {
        let mn = Vec3::new(0.0, 0.0, 0.0);
        let mx = Vec3::new(1.0, 1.0, 1.0);
        let (nice_mn, nice_mx) = nice_bounds(mn, mx);
        let labels = ["X".to_string(), "Y".to_string(), "Z".to_string()];
        let eye = Vec3::new(2.0, 2.0, 2.0);
        let geo = build_grid(
            mn, mx, nice_mn, nice_mx, [None; 3], [true; 3], eye, &labels, false, false, false, None,
        );
        assert!(!geo.vertices.is_empty(), "should produce box edges");
    }

    #[test]
    fn build_grid_with_planes_adds_more_vertices() {
        let mn = Vec3::new(0.0, 0.0, 0.0);
        let mx = Vec3::new(10.0, 10.0, 10.0);
        let (nice_mn, nice_mx) = nice_bounds(mn, mx);
        let labels = ["X".to_string(), "Y".to_string(), "Z".to_string()];
        let eye = Vec3::new(20.0, 20.0, 20.0);
        let geo_no_planes = build_grid(
            mn, mx, nice_mn, nice_mx, [None; 3], [true; 3], eye, &labels, false, false, false, None,
        );
        let geo_with_planes = build_grid(
            mn, mx, nice_mn, nice_mx, [None; 3], [true; 3], eye, &labels, true, false, false, None,
        );
        assert!(geo_with_planes.vertices.len() > geo_no_planes.vertices.len());
    }

    #[test]
    fn build_grid_all_edges_draws_stable_boundary_box() {
        let mn = Vec3::new(0.0, 0.0, 0.0);
        let mx = Vec3::new(10.0, 10.0, 10.0);
        let (nice_mn, nice_mx) = nice_bounds(mn, mx);
        let labels = ["X".to_string(), "Y".to_string(), "Z".to_string()];
        let eye = Vec3::new(20.0, 20.0, 20.0);
        let dynamic = build_grid(
            mn, mx, nice_mn, nice_mx, [None; 3], [true; 3], eye, &labels, false, false, false, None,
        );
        let all_edges = build_grid(
            mn, mx, nice_mn, nice_mx, [None; 3], [true; 3], eye, &labels, false, false, true, None,
        );
        assert!(all_edges.vertices.len() >= dynamic.vertices.len() + 24);
    }

    #[test]
    fn build_grid_z_hidden_suppresses_z_ticks() {
        let mn = Vec3::new(0.0, 0.0, 0.0);
        let mx = Vec3::new(10.0, 10.0, 0.001);
        let (nice_mn, nice_mx) = nice_bounds(mn, mx);
        let labels = ["X".to_string(), "Y".to_string(), "Z".to_string()];
        let eye = Vec3::new(5.0, -20.0, 5.0);
        let geo = build_grid(
            mn,
            mx,
            nice_mn,
            nice_mx,
            [None; 3],
            [true, true, false],
            eye,
            &labels,
            false,
            false,
            false,
            None,
        );
        // All labels should have "Z" as axis title or none — Z ticks suppressed
        let has_z_label = geo.labels.iter().any(|l| l.text == "Z" && l.is_axis_title);
        assert!(
            !has_z_label,
            "Z axis title should be suppressed when axis_visible[2]=false and axis_texts[2]='Z'"
        );
        // Actually with axis_visible[2]=false, the axis title for Z IS in axis_texts[2] but z_show=false so it's skipped
    }
}
