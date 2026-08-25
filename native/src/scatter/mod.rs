pub mod camera;
pub mod colormap;
pub mod grid;

use bytemuck::{Pod, Zeroable};
use camera::Camera;
use grid::{build_grid, stable_face_bits, sticky_nice_bounds, GridGeometry, LineVertex};
use std::{borrow::Cow, sync::Arc, time::Instant};

// ---------------------------------------------------------------------------
// GPU vertex layout
// ---------------------------------------------------------------------------

/// One point rendered as a screen-space billboard quad (4 vertices, instanced).
/// Matches @location attributes in points.wgsl.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PointInstance {
    pub position: [f32; 3], // @location(0)
    pub size: f32,          // @location(1)  pixels
    pub color: [f32; 3],    // @location(2)
    pub alpha: f32,         // @location(3)
}

const MAX_GPU_COLORMAP_POINTS: usize = 9;
const MIN_SCATTER_BUFFER_CAPACITY: u64 = 4 * 1024 * 1024;

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
#[error(
    "scatter capacity exceeded for {representation}: {requested_points} points require {required_bytes} bytes ({stride_bytes} bytes/point), but the device max_buffer_size is {max_buffer_size} bytes; use compact attributes, rendering='adaptive', or split the data into smaller exact chunks"
)]
pub struct ScatterCapacityError {
    pub requested_points: u64,
    pub required_bytes: u64,
    pub max_buffer_size: u64,
    pub stride_bytes: u64,
    pub representation: &'static str,
}

fn scatter_capacity_error(
    required_bytes: u64,
    max_buffer_size: u64,
    stride_bytes: u64,
    representation: &'static str,
) -> ScatterCapacityError {
    ScatterCapacityError {
        requested_points: required_bytes / stride_bytes.max(1),
        required_bytes,
        max_buffer_size,
        stride_bytes,
        representation,
    }
}

/// Choose reusable allocation headroom without crossing the adapter limit.
fn bounded_scatter_buffer_capacity(required: u64, max_buffer_size: u64) -> Option<u64> {
    if required == 0 || required > max_buffer_size {
        return None;
    }
    Some(
        required
            .saturating_mul(2)
            .max(MIN_SCATTER_BUFFER_CAPACITY.min(max_buffer_size))
            .min(max_buffer_size),
    )
}

fn scatter_chunk_layout(
    total_bytes: u64,
    max_buffer_size: u64,
    stride_bytes: u64,
) -> Option<(u64, usize)> {
    if total_bytes == 0 || stride_bytes == 0 || total_bytes % stride_bytes != 0 {
        return None;
    }
    let chunk_bytes = max_buffer_size / stride_bytes * stride_bytes;
    if chunk_bytes == 0 {
        return None;
    }
    Some((chunk_bytes, total_bytes.div_ceil(chunk_bytes) as usize))
}

fn compact_xyz_chunk_bounds(bytes: &[u8]) -> Option<(glam::Vec3, glam::Vec3)> {
    let mut min = glam::Vec3::splat(f32::INFINITY);
    let mut max = glam::Vec3::splat(f32::NEG_INFINITY);
    let mut any = false;
    for point in bytes.chunks_exact(12) {
        let value = glam::Vec3::new(
            f32::from_le_bytes(point[0..4].try_into().unwrap()),
            f32::from_le_bytes(point[4..8].try_into().unwrap()),
            f32::from_le_bytes(point[8..12].try_into().unwrap()),
        );
        if value.is_finite() {
            min = min.min(value);
            max = max.max(value);
            any = true;
        }
    }
    any.then_some((min, max))
}

fn aabb_intersects_clip(bounds: (glam::Vec3, glam::Vec3), view_proj: glam::Mat4) -> bool {
    let (min, max) = bounds;
    let mut clips = [glam::Vec4::ZERO; 8];
    let mut index = 0;
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                clips[index] = view_proj * glam::Vec4::new(x, y, z, 1.0);
                index += 1;
            }
        }
    }
    !clips.iter().all(|p| p.x < -p.w)
        && !clips.iter().all(|p| p.x > p.w)
        && !clips.iter().all(|p| p.y < -p.w)
        && !clips.iter().all(|p| p.y > p.w)
        && !clips.iter().all(|p| p.z < 0.0)
        && !clips.iter().all(|p| p.z > p.w)
}

static POINT_ATTRS: [wgpu::VertexAttribute; 4] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 12,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 16,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 28,
        shader_location: 3,
    },
];

fn point_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<PointInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &POINT_ATTRS,
    }
}

static XYZ_POINT_ATTRS: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
    format: wgpu::VertexFormat::Float32x3,
    offset: 0,
    shader_location: 0,
}];

fn xyz_point_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: 12,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &XYZ_POINT_ATTRS,
    }
}

static XY_POINT_ATTRS: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
    format: wgpu::VertexFormat::Float32x2,
    offset: 0,
    shader_location: 0,
}];

fn xy_point_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: 8,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &XY_POINT_ATTRS,
    }
}

fn points_are_opaque(points: &[PointInstance]) -> bool {
    points.iter().all(|point| point.alpha >= 0.999)
}

fn point_instance_bytes_are_opaque(bytes: &[u8]) -> bool {
    const ALPHA_OFFSET: usize = 28;
    const STRIDE: usize = std::mem::size_of::<PointInstance>();
    bytes.chunks_exact(STRIDE).all(|chunk| {
        let alpha = f32::from_le_bytes([
            chunk[ALPHA_OFFSET],
            chunk[ALPHA_OFFSET + 1],
            chunk[ALPHA_OFFSET + 2],
            chunk[ALPHA_OFFSET + 3],
        ]);
        alpha >= 0.999
    })
}

fn gpu_colormap_uniform(name: &str) -> ([[f32; 4]; MAX_GPU_COLORMAP_POINTS], u32) {
    let points = colormap::resolve(name);
    let mut out = [[0.0; 4]; MAX_GPU_COLORMAP_POINTS];
    let len = points.len().min(MAX_GPU_COLORMAP_POINTS).max(2);
    for i in 0..MAX_GPU_COLORMAP_POINTS {
        let source = points[i.min(points.len().saturating_sub(1))];
        out[i] = [source[0], source[1], source[2], 1.0];
    }
    (out, len as u32)
}

// ---------------------------------------------------------------------------
// Uniform block — matches Uniforms struct in points.wgsl
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, PartialEq)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    screen_size: [f32; 2],
    style: u32,
    point_size: f32,
    point_size_scale: f32,
    _pad0: [f32; 3],
    clip_radii: [f32; 4],
    compact_z_range: [f32; 2],
    compact_colormap_len: u32,
    point_color_mode: u32,
    compact_colormap: [[f32; 4]; MAX_GPU_COLORMAP_POINTS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrimaryPointStorage {
    PointInstance,
    XyF32,
    XyzF32,
}

// ---------------------------------------------------------------------------
// Chrome state: grid, axes, background
// ---------------------------------------------------------------------------

/// One entry in the categorical legend (label + swatch color).
#[derive(Clone, Debug, PartialEq)]
pub struct LegendEntry {
    pub label: String,
    pub color: [f32; 3],
}

/// Where the legend is anchored within the scatter viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LegendPosition {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl LegendPosition {
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "top-left" => Self::TopLeft,
            "bottom-right" => Self::BottomRight,
            "bottom-left" => Self::BottomLeft,
            _ => Self::TopRight,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LegendState {
    pub visible: bool,
    pub position: LegendPosition,
    pub entries: Vec<LegendEntry>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarBarState {
    pub visible: bool,
    pub vmin: f32,
    pub vmax: f32,
    pub log_scale: bool,
    pub colormap: String,
    pub title: Option<String>,
}

impl Default for ScalarBarState {
    fn default() -> Self {
        Self {
            visible: false,
            vmin: 0.0,
            vmax: 1.0,
            log_scale: false,
            colormap: "viridis".to_string(),
            title: None,
        }
    }
}

/// Format a scalar bar tick value for display.
/// `value` is always a raw-domain value (matching DragonSci public API).
/// For log-scale bars the value is displayed in scientific notation; for linear bars as decimal.
pub fn format_scalar_bar_tick(value: f32, log_scale: bool) -> String {
    if log_scale {
        format!("{:.2e}", value)
    } else {
        format!("{:.3}", value)
    }
}

/// Return up to `count` evenly-spaced tick values between `vmin` and `vmax` (inclusive).
/// For log-scale bars the spacing is even in ln-space but the returned values are raw domain.
/// Returns an empty vec when count < 2; returns a single-element vec when vmin == vmax.
pub fn scalar_bar_tick_values(vmin: f32, vmax: f32, log_scale: bool, count: usize) -> Vec<f32> {
    if count < 2 {
        return Vec::new();
    }
    if vmin == vmax {
        return vec![vmin];
    }
    if log_scale && vmin > 0.0 && vmax > 0.0 {
        let ln_min = vmin.ln();
        let ln_max = vmax.ln();
        (0..count)
            .map(|i| {
                let t = i as f32 / (count - 1) as f32;
                (ln_min + t * (ln_max - ln_min)).exp()
            })
            .collect()
    } else {
        (0..count)
            .map(|i| {
                let t = i as f32 / (count - 1) as f32;
                vmin + t * (vmax - vmin)
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScatterChromeState {
    pub grid_visible: bool,
    pub major_planes: bool,
    pub minor_planes: bool,
    pub grid_sticky: bool,
    pub grid_all_edges: bool,
    pub tick_override: [Option<usize>; 3],
    pub axis_labels: [String; 3],
    pub axis_visible: [bool; 3],
    pub background_color: Option<[f32; 4]>,
    pub legend: LegendState,
    pub scalar_bar: ScalarBarState,
    pub orientation_axes_visible: bool,
}

impl Default for ScatterChromeState {
    fn default() -> Self {
        Self {
            grid_visible: false,
            major_planes: false,
            minor_planes: false,
            grid_sticky: true,
            grid_all_edges: false,
            tick_override: [None; 3],
            axis_labels: ["X".to_string(), "Y".to_string(), "Z".to_string()],
            axis_visible: [true; 3],
            background_color: None,
            legend: LegendState::default(),
            scalar_bar: ScalarBarState::default(),
            orientation_axes_visible: false,
        }
    }
}

/// Which mouse interaction is active on a scatter widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PickingMode {
    /// Click a point to pick it (default).
    #[default]
    Point,
    /// Drag to draw a selection rectangle; release emits selected indices.
    Rectangle,
    /// Drag to draw a lasso path; release emits selected indices.
    Lasso,
    /// Interaction disabled — no pick or selection callbacks.
    None,
}

impl PickingMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "rectangle" => Self::Rectangle,
            "lasso" => Self::Lasso,
            "none" => Self::None,
            _ => Self::Point,
        }
    }
}

/// Mouse and depth behavior for scatter widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScatterInteractionMode {
    /// Left drag orbits the 3D camera and all points write depth.
    #[default]
    Orbit3D,
    /// Left drag pans a flat XY view and points render without depth writes.
    Pan2D,
}

impl ScatterInteractionMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "pan_2d" | "2d" | "flat" => Self::Pan2D,
            _ => Self::Orbit3D,
        }
    }
}

/// How a stream actor fills its fixed-capacity buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamMode {
    /// Fill from the start; stop writing when full.
    Append,
    /// Overwrite oldest points in a ring (never reallocates).
    Ring,
}

// ---------------------------------------------------------------------------
// ScreenPickCache
// ---------------------------------------------------------------------------

/// Pixel size of each grid cell in the screen-pick grid.
const PICK_CELL_SIZE: f32 = 32.0;

/// Project a 3D world-space position to viewport-local screen-space coordinates.
/// Returns `Some([sx, sy])` if the point is inside the view frustum, `None` if clipped.
fn project_to_screen(
    pos: [f32; 3],
    view_proj: &glam::Mat4,
    width: u32,
    height: u32,
) -> Option<[f32; 2]> {
    if !pos[0].is_finite() || !pos[1].is_finite() || !pos[2].is_finite() {
        return None;
    }
    let clip = *view_proj * glam::Vec4::new(pos[0], pos[1], pos[2], 1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 || ndc.z < 0.0 || ndc.z > 1.0 {
        return None;
    }
    Some([
        (ndc.x * 0.5 + 0.5) * width as f32,
        (0.5 - ndc.y * 0.5) * height as f32,
    ])
}

/// Lazily-built screen-space grid cache for O(radius²/cell²) pick queries.
///
/// Stores the projected viewport-local coordinates of all visible points in a
/// uniform grid indexed in CSR (Compressed Sparse Row) format.  The cache is
/// invalidated whenever the view–projection matrix or viewport dimensions change.
pub struct ScreenPickCache {
    /// View–projection matrix snapshot (invalidation key).
    view_proj: glam::Mat4,
    width: u32,
    height: u32,
    cell_size: f32,
    /// `ceil(width / cell_size)`.
    grid_cols: u32,
    /// `ceil(height / cell_size)`.
    grid_rows: u32,
    /// Projected viewport-local `[sx, sy]` for each source point.
    /// `[NaN, NaN]` means the point is clipped / behind the camera.
    _screen_xy: Vec<[f32; 2]>,
    /// CSR prefix sums: `cell_starts[c]..cell_starts[c+1]` indexes
    /// `sorted_indices` for grid cell `c`.
    cell_starts: Vec<u32>,
    /// Source point indices (into the original points slice) sorted by cell.
    sorted_indices: Vec<u32>,
    /// Maximum effective point size across all source points (accounts for global override).
    /// Used to expand the candidate query radius so large points are not missed.
    pub max_point_size: f32,
    /// Snapshot of the point_size_override used at build time (invalidation key).
    point_size_override: f32,
}

impl ScreenPickCache {
    /// Build a cache from `points` projected through `view_proj` onto a
    /// `width × height` viewport.
    ///
    /// `point_size_override` mirrors `ScatterWidget::point_size_override`:
    /// negative means use per-point sizes, non-negative overrides all sizes.
    pub fn build(
        points: &[PointInstance],
        view_proj: glam::Mat4,
        width: u32,
        height: u32,
        point_size_override: f32,
    ) -> Self {
        // Compute max effective point size for candidate radius expansion.
        let max_point_size = if point_size_override >= 0.0 {
            point_size_override
        } else {
            points.iter().map(|p| p.size).fold(0.0_f32, f32::max)
        };

        let cell_size = PICK_CELL_SIZE;
        let grid_cols = ((width as f32 / cell_size).ceil() as u32).max(1);
        let grid_rows = ((height as f32 / cell_size).ceil() as u32).max(1);
        let total_cells = (grid_cols * grid_rows) as usize;
        let n = points.len();

        let mut screen_xy = vec![[f32::NAN, f32::NAN]; n];
        let mut counts = vec![0u32; total_cells];

        for (i, pt) in points.iter().enumerate() {
            if let Some([sx, sy]) = project_to_screen(pt.position, &view_proj, width, height) {
                let cx = ((sx / cell_size) as u32).min(grid_cols - 1);
                let cy = ((sy / cell_size) as u32).min(grid_rows - 1);
                screen_xy[i] = [sx, sy];
                counts[(cx * grid_rows + cy) as usize] += 1;
            }
        }

        // Build CSR prefix sums.
        let mut cell_starts = vec![0u32; total_cells + 1];
        for c in 0..total_cells {
            cell_starts[c + 1] = cell_starts[c] + counts[c];
        }

        // Fill sorted_indices using counts as per-cell cursors.
        let total_visible = cell_starts[total_cells] as usize;
        let mut sorted_indices = vec![0u32; total_visible];
        counts.fill(0);

        for (i, &[sx, sy]) in screen_xy.iter().enumerate() {
            if sx.is_nan() {
                continue;
            }
            let cx = ((sx / cell_size) as u32).min(grid_cols - 1);
            let cy = ((sy / cell_size) as u32).min(grid_rows - 1);
            let c = (cx * grid_rows + cy) as usize;
            let slot = cell_starts[c] + counts[c];
            sorted_indices[slot as usize] = i as u32;
            counts[c] += 1;
        }

        Self {
            view_proj,
            width,
            height,
            cell_size,
            grid_cols,
            grid_rows,
            _screen_xy: screen_xy,
            cell_starts,
            sorted_indices,
            max_point_size,
            point_size_override,
        }
    }

    /// Returns `true` if the cached projection is outdated.
    pub fn is_stale(
        &self,
        view_proj: &glam::Mat4,
        width: u32,
        height: u32,
        point_size_override: f32,
    ) -> bool {
        self.width != width
            || self.height != height
            || &self.view_proj != view_proj
            || self.point_size_override.to_bits() != point_size_override.to_bits()
    }

    /// Collect candidate point indices whose grid cells overlap the search circle
    /// centred at `(local_x, local_y)` (viewport-local pixels) with radius `radius_px`.
    pub fn candidates_into(&self, local_x: f32, local_y: f32, radius_px: f32, out: &mut Vec<u32>) {
        out.clear();
        if self.grid_cols == 0 || self.grid_rows == 0 {
            return;
        }
        let cs = self.cell_size;
        let min_cx = ((local_x - radius_px) / cs).floor().max(0.0) as u32;
        let max_cx = (((local_x + radius_px) / cs).floor() as u32).min(self.grid_cols - 1);
        let min_cy = ((local_y - radius_px) / cs).floor().max(0.0) as u32;
        let max_cy = (((local_y + radius_px) / cs).floor() as u32).min(self.grid_rows - 1);

        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                let c = (cx * self.grid_rows + cy) as usize;
                if c + 1 < self.cell_starts.len() {
                    let start = self.cell_starts[c] as usize;
                    let end = self.cell_starts[c + 1] as usize;
                    out.extend_from_slice(&self.sorted_indices[start..end]);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn candidates(&self, local_x: f32, local_y: f32, radius_px: f32) -> Vec<u32> {
        let mut out = Vec::new();
        self.candidates_into(local_x, local_y, radius_px, &mut out);
        out
    }
}

// ---------------------------------------------------------------------------
// PointActor
// ---------------------------------------------------------------------------

/// One independently managed point set within a scatter scene.
pub struct PointActor {
    _id: u32,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_cap: u64,
    /// Hash-sorted copy for representative LOD sampling; None for stream actors.
    lod_vertex_buffer: Option<wgpu::Buffer>,
    lod_vertex_cap: u64,
    lod_sampled_scratch: Vec<PointInstance>,
    lod_bucket_keys_scratch: Vec<u32>,
    lod_occupied_scratch: Vec<bool>,
    pub point_count: u32,
    points_opaque: bool,
    pub points: Vec<PointInstance>,
    pub visible: bool,
    pub data_min: glam::Vec3,
    pub data_max: glam::Vec3,
    /// Set only for stream actors; holds fixed max capacity and ring cursor.
    pub stream_mode: Option<StreamMode>,
    pub stream_capacity: u32,
    pub stream_write_offset: u32,
    /// Per-point tooltip text. Empty vec = use coordinate fallback for all points.
    pub hover_meta: Vec<String>,
    /// Source column names used as coordinate labels in hover tooltips.
    pub tooltip_axis_labels: [String; 3],
    /// Lazily-built screen-space pick cache. `None` = needs rebuild.
    pub pick_cache: Option<ScreenPickCache>,
}

impl PointActor {
    fn new(id: u32) -> Self {
        Self {
            _id: id,
            vertex_buffer: None,
            vertex_cap: 0,
            lod_vertex_buffer: None,
            lod_vertex_cap: 0,
            lod_sampled_scratch: Vec::new(),
            lod_bucket_keys_scratch: Vec::new(),
            lod_occupied_scratch: Vec::new(),
            point_count: 0,
            points_opaque: true,
            points: Vec::new(),
            visible: true,
            data_min: glam::Vec3::splat(f32::MAX),
            data_max: glam::Vec3::splat(f32::MIN),
            stream_mode: None,
            stream_capacity: 0,
            stream_write_offset: 0,
            hover_meta: Vec::new(),
            tooltip_axis_labels: ["x".to_string(), "y".to_string(), "z".to_string()],
            pick_cache: None,
        }
    }

    fn upload(
        &mut self,
        pts: &[PointInstance],
        build_lod: bool,
        lod_factor: u32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> ScatterUploadTimings {
        // Invalidate pick cache whenever point data changes.
        self.pick_cache = None;
        let size = (pts.len() * std::mem::size_of::<PointInstance>()) as u64;
        if size == 0 {
            self.point_count = 0;
            self.points_opaque = true;
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
            return ScatterUploadTimings::default();
        }
        if self.vertex_buffer.is_none() || size > self.vertex_cap {
            let Some(cap) = bounded_scatter_buffer_capacity(size, device.limits().max_buffer_size)
            else {
                self.vertex_buffer = None;
                self.vertex_cap = 0;
                self.point_count = 0;
                return ScatterUploadTimings::default();
            };
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-actor-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_cap = cap;
        }
        let primary_t0 = Instant::now();
        queue.write_buffer(
            self.vertex_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(pts),
        );
        let primary_ms = primary_t0.elapsed().as_secs_f64() * 1000.0;
        self.point_count = pts.len() as u32;
        self.points_opaque = points_are_opaque(pts);
        let mut lod_ms = 0.0;
        if self.stream_mode.is_none() && build_lod {
            let lod_t0 = Instant::now();
            upload_lod_buffer(
                pts,
                lod_factor,
                &mut self.lod_vertex_buffer,
                &mut self.lod_vertex_cap,
                device,
                queue,
                &mut self.lod_sampled_scratch,
                &mut self.lod_bucket_keys_scratch,
                &mut self.lod_occupied_scratch,
            );
            lod_ms = lod_t0.elapsed().as_secs_f64() * 1000.0;
        } else if self.stream_mode.is_none() {
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
        }
        ScatterUploadTimings { primary_ms, lod_ms }
    }

    fn compute_bounds(pts: &[PointInstance]) -> (glam::Vec3, glam::Vec3) {
        let mut mn = glam::Vec3::splat(f32::MAX);
        let mut mx = glam::Vec3::splat(f32::MIN);
        for p in pts {
            let v = glam::Vec3::from_array(p.position);
            if v.is_finite() {
                mn = mn.min(v);
                mx = mx.max(v);
            }
        }
        if mn.x > mx.x {
            (glam::Vec3::ZERO, glam::Vec3::ZERO)
        } else {
            (mn, mx)
        }
    }
}

/// World-space label projected to the scatter viewport's screen space.
pub struct ProjectedLabel {
    pub screen_x: f32,
    pub screen_y: f32,
    pub text: String,
    pub is_title: bool,
    /// Explicit text color (0.0–1.0). `None` uses the grid default (#bbb).
    pub color: Option<[f32; 3]>,
    /// Explicit font size in logical pixels. `None` uses the grid default (11/13 px).
    pub font_size: Option<f32>,
    /// Text anchor: `"left"`, `"center"`, `"right"`, or internal `"top-left"`.
    pub anchor: Cow<'static, str>,
}

struct GridLabelProjection {
    local_x: f32,
    local_y: f32,
    text: String,
    is_title: bool,
    push_dir: glam::Vec2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScalarBarVertexCacheKey {
    width: u32,
    height: u32,
    colormap: String,
}

#[derive(Clone, Debug, PartialEq)]
struct GridLabelProjectionCacheKey {
    view_proj: glam::Mat4,
    width: u32,
    height: u32,
    offset: [f32; 2],
    scissor_offset: [u32; 2],
    scissor_size: [u32; 2],
}

fn estimate_scatter_label_size(text: &str, is_title: bool) -> glam::Vec2 {
    let font_size = if is_title { 13.0_f32 } else { 11.0_f32 };
    let line_count = text.lines().count().max(1) as f32;
    let max_chars = text
        .lines()
        .map(scatter_label_char_count)
        .max()
        .unwrap_or_else(|| scatter_label_char_count(text))
        .max(1) as f32;
    let max_width = if is_title { 200.0_f32 } else { 120.0_f32 };
    glam::Vec2::new(
        (max_chars * font_size * 0.62 + 2.0).min(max_width),
        line_count * font_size * 1.3,
    )
}

fn scatter_label_char_count(text: &str) -> usize {
    if text.is_ascii() {
        text.len()
    } else {
        text.chars().count()
    }
}

fn scatter_label_rect(local_x: f32, local_y: f32, text: &str, is_title: bool) -> [f32; 4] {
    let size = estimate_scatter_label_size(text, is_title);
    [
        local_x - size.x * 0.5,
        local_y - size.y * 0.5,
        local_x + size.x * 0.5,
        local_y + size.y * 0.5,
    ]
}

fn scatter_rects_overlap(a: [f32; 4], b: [f32; 4], gap: f32) -> bool {
    a[0] < b[2] + gap && a[2] + gap > b[0] && a[1] < b[3] + gap && a[3] + gap > b[1]
}

fn push_scatter_title_away_from_rects(
    mut local_x: f32,
    mut local_y: f32,
    text: &str,
    mut push_dir: glam::Vec2,
    obstacles: &[[f32; 4]],
) -> (f32, f32) {
    const LABEL_GAP_PX: f32 = 8.0;
    const PUSH_STEP_PX: f32 = 6.0;
    const MAX_STEPS: usize = 24;

    if push_dir.length_squared() < 1.0 {
        push_dir = glam::Vec2::new(0.0, -1.0);
    } else {
        push_dir = push_dir.normalize();
    }

    for _ in 0..MAX_STEPS {
        let rect = scatter_label_rect(local_x, local_y, text, true);
        if !obstacles
            .iter()
            .any(|&obstacle| scatter_rects_overlap(rect, obstacle, LABEL_GAP_PX))
        {
            break;
        }
        local_x += push_dir.x * PUSH_STEP_PX;
        local_y += push_dir.y * PUSH_STEP_PX;
    }

    (local_x, local_y)
}

/// A world-space text label added by the user via `add_label`.
pub struct UserLabel {
    pub id: u32,
    pub position: glam::Vec3,
    pub text: String,
    pub color: [f32; 3],
    pub size: f32,
    pub anchor: String,
    pub visible: bool,
}

/// A set of world-space line segments added by the user via `add_lines` / `add_box`.
pub struct LineOverlay {
    pub id: u32,
    /// Paired endpoints: even indices = starts, odd = ends.
    pub vertices: Vec<LineVertex>,
    pub visible: bool,
}

// ---------------------------------------------------------------------------
// Mesh (hull / ellipsoid) actor
// ---------------------------------------------------------------------------

/// One vertex in a mesh overlay — position + RGBA color.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

static MESH_VERTEX_ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 12,
        shader_location: 1,
    },
];

/// An independently managed mesh overlay (convex hull, ellipsoid, etc.).
pub struct MeshActor {
    _id: u32,
    pub visible: bool,
    pub wireframe: bool,
    pub color: [f32; 4],
    /// Retained for style-only updates (color/wireframe toggle without re-sending geometry).
    pub positions: Vec<[f32; 3]>,
    pub triangle_indices: Vec<[u32; 3]>,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    pub index_count: u32,
    pub primitive_topology: wgpu::PrimitiveTopology,
    pub data_min: glam::Vec3,
    pub data_max: glam::Vec3,
}

impl MeshActor {
    pub fn new(
        id: u32,
        positions: Vec<[f32; 3]>,
        triangle_indices: Vec<[u32; 3]>,
        color: [f32; 4],
        wireframe: bool,
        device: &wgpu::Device,
    ) -> Self {
        let mut actor = Self {
            _id: id,
            visible: true,
            wireframe,
            color,
            positions,
            triangle_indices,
            vertex_buffer: None,
            index_buffer: None,
            index_count: 0,
            primitive_topology: if wireframe {
                wgpu::PrimitiveTopology::LineList
            } else {
                wgpu::PrimitiveTopology::TriangleList
            },
            data_min: glam::Vec3::ZERO,
            data_max: glam::Vec3::ZERO,
        };
        actor.rebuild_buffers(device);
        actor
    }

    /// Rebuild vertex and index GPU buffers from stored positions/triangles.
    pub fn rebuild_buffers(&mut self, device: &wgpu::Device) {
        use wgpu::util::DeviceExt;
        self.vertex_buffer = None;
        self.index_buffer = None;
        self.index_count = 0;
        if self.positions.is_empty() || self.triangle_indices.is_empty() {
            return;
        }
        let verts: Vec<MeshVertex> = self
            .positions
            .iter()
            .map(|&pos| MeshVertex {
                position: pos,
                color: self.color,
            })
            .collect();
        let indices: Vec<u32> = if self.wireframe {
            triangles_to_wireframe_indices(&self.triangle_indices)
        } else {
            self.triangle_indices
                .iter()
                .flat_map(|&[a, b, c]| [a, b, c])
                .collect()
        };
        self.index_count = indices.len() as u32;
        self.primitive_topology = if self.wireframe {
            wgpu::PrimitiveTopology::LineList
        } else {
            wgpu::PrimitiveTopology::TriangleList
        };
        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh-verts"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
        );
        self.index_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh-indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            }),
        );
        let mut mn = glam::Vec3::splat(f32::MAX);
        let mut mx = glam::Vec3::splat(f32::MIN);
        for &p in &self.positions {
            let v = glam::Vec3::from_array(p);
            mn = mn.min(v);
            mx = mx.max(v);
        }
        if mn.x <= mx.x {
            self.data_min = mn;
            self.data_max = mx;
        }
    }

    pub fn render_into<'pass, 'data: 'pass>(&'data self, pass: &mut wgpu::RenderPass<'pass>) {
        if !self.visible || self.index_count == 0 {
            return;
        }
        let (Some(vb), Some(ib)) = (self.vertex_buffer.as_ref(), self.index_buffer.as_ref()) else {
            return;
        };
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

/// Convert triangle indices to a deduplicated edge index list for wireframe rendering.
fn triangles_to_wireframe_indices(tris: &[[u32; 3]]) -> Vec<u32> {
    use std::collections::HashSet;
    let mut edges: HashSet<(u32, u32)> = HashSet::new();
    let mut out = Vec::with_capacity(tris.len() * 6);
    for &[a, b, c] in tris {
        for (u, v) in [(a, b), (b, c), (c, a)] {
            let key = if u < v { (u, v) } else { (v, u) };
            if edges.insert(key) {
                out.push(u);
                out.push(v);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Line vertex buffer layout
// ---------------------------------------------------------------------------

static LINE_ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 12,
        shader_location: 1,
    },
];

/// 2D overlay vertex: NDC position + RGB color.
/// Used by legend swatches, scalar bar strip, and orientation axes.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct OverlayVertex {
    pub position: [f32; 2], // NDC x, y  (-1..1)
    pub color: [f32; 3],
}

static OVERLAY_ATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 8,
        shader_location: 1,
    },
];

fn overlay_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<OverlayVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &OVERLAY_ATTRS,
    }
}

fn overlay_to_ndc(w: f32, h: f32, px: f32, py: f32) -> [f32; 2] {
    let nx = (px / w) * 2.0 - 1.0;
    let ny = 1.0 - (py / h) * 2.0;
    [nx, ny]
}

const SCALAR_BAR_WIDTH_PX: f32 = 16.0;
const SCALAR_BAR_TICK_GAP_PX: f32 = 6.0;
const SCALAR_BAR_RIGHT_PADDING_PX: f32 = 12.0;
const SCALAR_BAR_TICK_LABEL_WIDTH_PX: f32 = 58.0;
const SCALAR_BAR_RIGHT_GUTTER_PX: f32 =
    SCALAR_BAR_TICK_GAP_PX + SCALAR_BAR_TICK_LABEL_WIDTH_PX + SCALAR_BAR_RIGHT_PADDING_PX;
const SCALAR_BAR_TOP_PX: f32 = 40.0;
const SCALAR_BAR_TITLE_TOP_PX: f32 = 10.0;
const SCALAR_BAR_TITLE_RIGHT_INSET_PX: f32 = 8.0;

fn scalar_bar_geometry(w: f32, h: f32) -> (f32, f32, f32, f32) {
    let bar_h = (h * 0.45).min(220.0).max(60.0);
    let bar_x1 = w - SCALAR_BAR_RIGHT_GUTTER_PX - SCALAR_BAR_WIDTH_PX;
    let bar_x2 = w - SCALAR_BAR_RIGHT_GUTTER_PX;
    (bar_x1, bar_x2, SCALAR_BAR_TOP_PX, bar_h)
}

fn push_scalar_bar_vertices(w: f32, h: f32, colormap: &str, verts: &mut Vec<OverlayVertex>) {
    let (bar_x1, bar_x2, bar_top, bar_h) = scalar_bar_geometry(w, h);
    let bar_bottom: f32 = bar_top + bar_h;
    let n_strips: usize = 64;
    let cmap = colormap::resolve(colormap);
    for i in 0..n_strips {
        let t0 = i as f32 / n_strips as f32;
        let t1 = (i + 1) as f32 / n_strips as f32;
        let t_mid = (t0 + t1) * 0.5;
        let [r, g, b] = colormap::sample(cmap, 1.0 - t_mid);
        let y0 = bar_top + t0 * bar_h;
        let y1 = bar_top + t1 * bar_h;
        verts.push(OverlayVertex {
            position: overlay_to_ndc(w, h, bar_x1, y0),
            color: [r, g, b],
        });
        verts.push(OverlayVertex {
            position: overlay_to_ndc(w, h, bar_x2, y0),
            color: [r, g, b],
        });
        verts.push(OverlayVertex {
            position: overlay_to_ndc(w, h, bar_x1, y1),
            color: [r, g, b],
        });
        verts.push(OverlayVertex {
            position: overlay_to_ndc(w, h, bar_x2, y1),
            color: [r, g, b],
        });
    }

    let outline_col = [0.6f32, 0.6, 0.6];
    let corners = [
        overlay_to_ndc(w, h, bar_x1, bar_top),
        overlay_to_ndc(w, h, bar_x2, bar_top),
        overlay_to_ndc(w, h, bar_x2, bar_bottom),
        overlay_to_ndc(w, h, bar_x1, bar_bottom),
    ];
    for j in 0..4usize {
        verts.push(OverlayVertex {
            position: corners[j],
            color: outline_col,
        });
        verts.push(OverlayVertex {
            position: corners[(j + 1) % 4],
            color: outline_col,
        });
    }
}

fn line_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LineVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &LINE_ATTRS,
    }
}

fn point_size_override_value(point_size: Option<f32>) -> f32 {
    point_size.map(|size| size.max(0.0)).unwrap_or(-1.0)
}

fn adaptive_point_size_scale(point_count: u32, width: u32, height: u32) -> f32 {
    if point_count == 0 || width == 0 || height == 0 {
        return 1.0;
    }
    let pixels = (width as f32 * height as f32).max(1.0);
    let density = point_count as f32 / pixels;
    if density <= 0.08 {
        1.0
    } else if density <= 0.40 {
        let t = (density - 0.08) / (0.40 - 0.08);
        1.0 + (0.55 - 1.0) * t
    } else if density <= 0.90 {
        let t = (density - 0.40) / (0.90 - 0.40);
        0.55 + (0.35 - 0.55) * t
    } else {
        0.35
    }
}

fn fitted_camera_distance(radius: f32, aspect: f32, fov_y: f32) -> f32 {
    if !radius.is_finite() || !aspect.is_finite() || !fov_y.is_finite() || radius <= 0.0 {
        return 1.0;
    }
    let safe_aspect = aspect.max(0.001);
    let vertical_fit = radius / (fov_y * 0.5).tan().max(0.001);
    let horizontal_fit = vertical_fit / safe_aspect;
    vertical_fit.max(horizontal_fit) * 1.035
}

fn zoom_out_point_size_scale(camera_distance: f32, fit_distance: f32) -> f32 {
    if !camera_distance.is_finite() || !fit_distance.is_finite() || fit_distance <= 0.0 {
        return 1.0;
    }
    let ratio = (camera_distance / fit_distance).max(0.0);
    if ratio <= 1.25 {
        return 1.0;
    }
    if ratio >= 8.0 {
        return 0.45;
    }
    let t = ((ratio - 1.25) / (8.0 - 1.25)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 + (0.45 - 1.0) * smooth
}

fn pan2d_cursor_anchor_adjustment(
    before: [f32; 4],
    after: [f32; 4],
    pointer_fraction: [f32; 2],
) -> glam::Vec2 {
    let fx = pointer_fraction[0].clamp(0.0, 1.0);
    let fy_data = 1.0 - pointer_fraction[1].clamp(0.0, 1.0);
    let before_anchor = glam::Vec2::new(
        before[0] + (before[2] - before[0]) * fx,
        before[1] + (before[3] - before[1]) * fy_data,
    );
    let after_anchor = glam::Vec2::new(
        after[0] + (after[2] - after[0]) * fx,
        after[1] + (after[3] - after[1]) * fy_data,
    );
    before_anchor - after_anchor
}

fn clamp_interactive_render_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.clamp(0.25, 1.0)
    } else {
        1.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScatterLayoutRect {
    offset: [f32; 2],
    width: u32,
    height: u32,
    scissor_offset: [u32; 2],
    scissor_size: [u32; 2],
}

fn scatter_layout_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    visible_clip: Option<[f32; 4]>,
) -> ScatterLayoutRect {
    let width = w.max(0.0) as u32;
    let height = h.max(0.0) as u32;
    let [clip_x, clip_y, clip_w, clip_h] = visible_clip.unwrap_or([x, y, w, h]);
    let left = clip_x.max(x);
    let top = clip_y.max(y);
    let right = (clip_x + clip_w).min(x + w).max(left);
    let bottom = (clip_y + clip_h).min(y + h).max(top);
    let scissor_x = left.floor().max(0.0) as u32;
    let scissor_y = top.floor().max(0.0) as u32;
    let scissor_right = right.ceil().max(scissor_x as f32) as u32;
    let scissor_bottom = bottom.ceil().max(scissor_y as f32) as u32;
    ScatterLayoutRect {
        offset: [x, y],
        width,
        height,
        scissor_offset: [scissor_x, scissor_y],
        scissor_size: [
            scissor_right.saturating_sub(scissor_x),
            scissor_bottom.saturating_sub(scissor_y),
        ],
    }
}

fn scatter_scissor_bounds(offset: [u32; 2], size: [u32; 2]) -> [f32; 4] {
    [
        offset[0] as f32,
        offset[1] as f32,
        offset[0].saturating_add(size[0]) as f32,
        offset[1].saturating_add(size[1]) as f32,
    ]
}

fn scatter_points_write_depth(mode: ScatterInteractionMode) -> bool {
    mode == ScatterInteractionMode::Orbit3D
}

fn clamp_scissor_to_target(
    offset: [u32; 2],
    size: [u32; 2],
    target_width: u32,
    target_height: u32,
) -> Option<([u32; 2], [u32; 2])> {
    let left = offset[0].min(target_width);
    let top = offset[1].min(target_height);
    let right = offset[0].saturating_add(size[0]).min(target_width);
    let bottom = offset[1].saturating_add(size[1]).min(target_height);
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    (width > 0 && height > 0).then_some(([left, top], [width, height]))
}

// ---------------------------------------------------------------------------
// ScatterWidget
// ---------------------------------------------------------------------------

pub struct ScatterWidget {
    pipeline: wgpu::RenderPipeline,
    opaque_pipeline: Option<wgpu::RenderPipeline>,
    compact_pipeline: Option<wgpu::RenderPipeline>,
    compact_xy_pipeline: Option<wgpu::RenderPipeline>,
    compact_opaque_pipeline: Option<wgpu::RenderPipeline>,
    clip_mask_pipeline: Option<wgpu::RenderPipeline>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_cap: u64,
    primary_extra_buffers: Vec<wgpu::Buffer>,
    primary_extra_caps: Vec<u64>,
    primary_draw_counts: Vec<u32>,
    primary_source_offsets: Vec<u32>,
    primary_chunk_bounds: Vec<Option<(glam::Vec3, glam::Vec3)>>,
    primary_chunk_culling: bool,
    shared_primary: Option<Arc<SharedPointBuffers>>,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pub point_count: u32,
    primary_points_opaque: bool,
    primary_storage: PrimaryPointStorage,
    pub camera: Camera,
    /// Viewport offset within the window (pixels, top-left origin).
    pub offset: [f32; 2],
    pub width: u32,
    pub height: u32,
    scissor_offset: [u32; 2],
    scissor_size: [u32; 2],
    /// Saved for camera reset (R / Home).
    fit_center: glam::Vec3,
    fit_radius: f32,
    pub(crate) point_size_override: f32,
    pub auto_point_size: bool,
    pub(crate) point_size_scale: f32,
    pub interactive_render_scale: f32,
    pub auto_quality_enabled: bool,
    pub quality_target_frame_ms: f32,
    pub quality_level: u32,
    pub interaction_mode: ScatterInteractionMode,
    point_style: u32,
    clip_radii: [f32; 4],
    compact_z_range: [f32; 2],
    compact_colormap_len: u32,
    point_color_mode: u32,
    compact_colormap: [[f32; 4]; MAX_GPU_COLORMAP_POINTS],
    // ── Grid / chrome ────────────────────────────────────────────────────────
    line_pipeline: Option<wgpu::RenderPipeline>,
    line_vertex_buffer: Option<wgpu::Buffer>,
    line_vertex_cap: u64,
    pub grid_vertex_count: u32,
    pub chrome: ScatterChromeState,
    /// World-space grid label anchors from the last grid geometry rebuild.
    grid_labels: Vec<grid::LabelAnchor>,
    /// Grid, overlay, and annotation labels projected to screen space.
    pub pending_labels: Vec<ProjectedLabel>,
    /// Number of grid-axis labels at the front of `pending_labels`; overlay labels follow.
    pending_overlay_offset: usize,
    reproject_overlay_scratch: Vec<ProjectedLabel>,
    grid_label_rects_scratch: Vec<[f32; 4]>,
    grid_title_projection_scratch: Vec<GridLabelProjection>,
    last_face_bits: u8,
    last_grid_bounds: Option<(glam::Vec3, glam::Vec3)>,
    pub(crate) grid_display_bounds: Option<(glam::Vec3, glam::Vec3)>,
    last_grid_ortho_scale: Option<(f32, f32)>,
    last_grid_label_projection_key: Option<GridLabelProjectionCacheKey>,
    pub chrome_dirty: bool,
    // ── 2D screen-space overlays (legend, scalar bar, orientation axes) ──────
    overlay_pipeline: Option<wgpu::RenderPipeline>,
    overlay_vertex_buffer: Option<wgpu::Buffer>,
    overlay_vertex_cap: u64,
    pub overlay_vertex_count: u32,
    /// TriangleList pipeline for background fill quad — respects viewport/scissor.
    bg_pipeline: Option<wgpu::RenderPipeline>,
    bg_vertex_buffer: Option<wgpu::Buffer>,
    bg_vertex_cap: u64,
    bg_vertex_count: u32,
    overlay_vertices_scratch: Vec<OverlayVertex>,
    scalar_bar_vertex_cache_key: Option<ScalarBarVertexCacheKey>,
    scalar_bar_vertex_cache: Vec<OverlayVertex>,
    // ── Multi-actor point layers ──────────────────────────────────────────────
    /// Additional independently rendered point actors (actors beyond the legacy single buffer).
    pub extra_actors: std::collections::HashMap<u32, PointActor>,
    // ── LOD (Level of Detail during interaction) ─────────────────────────────
    pub lod_enabled: bool,
    pub lod_threshold: u32,
    pub lod_factor: u32,
    pub lod_active: bool,
    /// Hash-sorted copy of the main vertex buffer for representative LOD sampling.
    lod_vertex_buffer: Option<wgpu::Buffer>,
    lod_vertex_cap: u64,
    lod_sampled_scratch: Vec<PointInstance>,
    lod_bucket_keys_scratch: Vec<u32>,
    lod_occupied_scratch: Vec<bool>,
    // ── Interaction / picking mode ────────────────────────────────────────────
    pub picking_mode: PickingMode,
    // ── Selection rectangle overlay ──────────────────────────────────────────
    /// Viewport-local pixel rect (x0, y0, x1, y1) drawn during rectangle drag selection.
    pub selection_rect: Option<[f32; 4]>,
    /// Viewport-local pixel path accumulated during freehand lasso drag.
    pub selection_polygon: Option<Vec<[f32; 2]>>,
    /// Floating coordinate tooltip shown when hover_tooltip is enabled.
    pub hover_label: Option<ProjectedLabel>,
    // ── User annotations (labels, line overlays, boxes) ──────────────────────
    pub user_labels: Vec<UserLabel>,
    pub line_overlays: Vec<LineOverlay>,
    user_line_vertex_buffer: Option<wgpu::Buffer>,
    user_line_vertex_cap: u64,
    pub user_line_vertex_count: u32,
    // ── Mesh overlays (hulls, ellipsoids) ─────────────────────────────────────
    mesh_solid_pipeline: Option<wgpu::RenderPipeline>,
    mesh_transparent_pipeline: Option<wgpu::RenderPipeline>,
    mesh_wire_pipeline: Option<wgpu::RenderPipeline>,
    pub mesh_actors: std::collections::HashMap<u32, MeshActor>,
    // ── Phase 7: Screenshot cache ─────────────────────────────────────────────
    surface_format: wgpu::TextureFormat,
    screenshot_cache: Option<ScreenshotCache>,
    scene_revision: u64,
    last_uniforms: Option<Uniforms>,
    last_capacity_error: Option<ScatterCapacityError>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ScatterUploadTimings {
    pub primary_ms: f64,
    pub lod_ms: f64,
}

pub(crate) struct SharedPointBuffers {
    buffers: Vec<wgpu::Buffer>,
    capacities: Vec<u64>,
    draw_counts: Vec<u32>,
    source_offsets: Vec<u32>,
    chunk_bounds: Vec<Option<(glam::Vec3, glam::Vec3)>>,
    chunk_culling: bool,
    point_count: u32,
    storage: PrimaryPointStorage,
    points_opaque: bool,
}

impl SharedPointBuffers {
    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.capacities.iter().copied().sum()
    }

    pub(crate) fn used_bytes(&self) -> u64 {
        let stride = match self.storage {
            PrimaryPointStorage::PointInstance => std::mem::size_of::<PointInstance>() as u64,
            PrimaryPointStorage::XyF32 => 8,
            PrimaryPointStorage::XyzF32 => 12,
        };
        self.draw_counts
            .iter()
            .map(|count| *count as u64 * stride)
            .sum()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScatterGpuMemoryStats {
    pub primary_allocated_bytes: u64,
    pub primary_used_bytes: u64,
    pub primary_shared: bool,
    pub lod_allocated_bytes: u64,
    pub lod_used_bytes: u64,
    pub actor_allocated_bytes: u64,
    pub chrome_allocated_bytes: u64,
    pub mesh_allocated_bytes: u64,
    pub uniform_allocated_bytes: u64,
    pub total_allocated_bytes: u64,
}

struct ScreenshotCache {
    w: u32,
    h: u32,
    color_tex: wgpu::Texture,
    color_view: wgpu::TextureView,
    _depth_tex: wgpu::Texture,
    depth_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded_row: u32,
}

/// Ray-casting even-odd point-in-polygon test (2D screen coordinates).
fn point_in_polygon(x: f32, y: f32, poly: &[[f32; 2]]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let [xi, yi] = poly[i];
        let [xj, yj] = poly[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Murmur3-style hash of a point's position bits — used to sort the LOD buffer
/// so that the first N points give an even spatial sample instead of a prefix cluster.
fn lod_sort_key(p: &PointInstance) -> u32 {
    let xb = p.position[0].to_bits();
    let yb = p.position[1].to_bits();
    let zb = p.position[2].to_bits();
    let h = xb ^ yb.rotate_left(11) ^ zb.rotate_left(22);
    let h = h ^ (h >> 16);
    let h = h.wrapping_mul(0x85ebca6b_u32);
    let h = h ^ (h >> 13);
    let h = h.wrapping_mul(0xc2b2ae35_u32);
    h ^ (h >> 16)
}

fn lod_sample_count(point_count: usize, lod_factor: u32) -> usize {
    if point_count == 0 {
        return 0;
    }
    let factor = (lod_factor as usize).max(1);
    (point_count / factor).max(1).min(point_count)
}

/// Allocate (or reuse) `lod_buf`/`lod_cap` and write a hash-selected LOD sample of `pts`.
/// The three scratch Vecs are reused across calls to avoid per-frame heap allocation.
fn upload_lod_buffer(
    pts: &[PointInstance],
    lod_factor: u32,
    lod_buf: &mut Option<wgpu::Buffer>,
    lod_cap: &mut u64,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    sampled_scratch: &mut Vec<PointInstance>,
    bucket_keys_scratch: &mut Vec<u32>,
    occupied_scratch: &mut Vec<bool>,
) {
    let sample_count = lod_sample_count(pts.len(), lod_factor);
    let size = (sample_count * std::mem::size_of::<PointInstance>()) as u64;
    if size == 0 {
        return;
    }
    if lod_buf.is_none() || size > *lod_cap {
        let Some(cap) = bounded_scatter_buffer_capacity(size, device.limits().max_buffer_size)
        else {
            *lod_buf = None;
            *lod_cap = 0;
            return;
        };
        *lod_buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scatter-lod-vb"),
            size: cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        *lod_cap = cap;
    }
    if sample_count >= pts.len() {
        queue.write_buffer(lod_buf.as_ref().unwrap(), 0, bytemuck::cast_slice(pts));
        return;
    }

    // Grow scratch buffers if needed (never shrink — capacity is reused on future calls).
    if sampled_scratch.len() < sample_count {
        sampled_scratch.resize(sample_count, PointInstance::zeroed());
    }
    if bucket_keys_scratch.len() < sample_count {
        bucket_keys_scratch.resize(sample_count, u32::MAX);
    }
    // Always reset the first sample_count slots — old data is stale.
    bucket_keys_scratch[..sample_count].fill(u32::MAX);
    if occupied_scratch.len() < sample_count {
        occupied_scratch.resize(sample_count, false);
    }
    occupied_scratch[..sample_count].fill(false);

    let sampled = &mut sampled_scratch[..sample_count];
    let bucket_keys = &mut bucket_keys_scratch[..sample_count];
    let occupied = &mut occupied_scratch[..sample_count];

    let mut occupied_count = 0usize;
    for point in pts {
        let key = lod_sort_key(point);
        let bucket = ((key as u64 * sample_count as u64) >> 32) as usize;
        if !occupied[bucket] {
            occupied[bucket] = true;
            occupied_count += 1;
        }
        if key < bucket_keys[bucket] {
            bucket_keys[bucket] = key;
            sampled[bucket] = *point;
        }
    }

    if occupied_count < sample_count {
        let stride = (pts.len() / sample_count).max(1);
        let mut src = 0usize;
        for (i, is_occupied) in occupied.iter().copied().enumerate() {
            if is_occupied {
                continue;
            }
            sampled[i] = pts[src.min(pts.len() - 1)];
            src = (src + stride).min(pts.len() - 1);
        }
    }
    queue.write_buffer(lod_buf.as_ref().unwrap(), 0, bytemuck::cast_slice(sampled));
}

fn stencil_face(
    compare: wgpu::CompareFunction,
    pass_op: wgpu::StencilOperation,
) -> wgpu::StencilFaceState {
    wgpu::StencilFaceState {
        compare,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op,
    }
}

fn scatter_scene_stencil_state() -> wgpu::StencilState {
    wgpu::StencilState {
        front: stencil_face(wgpu::CompareFunction::Equal, wgpu::StencilOperation::Keep),
        back: stencil_face(wgpu::CompareFunction::Equal, wgpu::StencilOperation::Keep),
        read_mask: 0xff,
        write_mask: 0x00,
    }
}

fn scatter_clip_mask_stencil_state() -> wgpu::StencilState {
    wgpu::StencilState {
        front: stencil_face(
            wgpu::CompareFunction::Always,
            wgpu::StencilOperation::Replace,
        ),
        back: stencil_face(
            wgpu::CompareFunction::Always,
            wgpu::StencilOperation::Replace,
        ),
        read_mask: 0xff,
        write_mask: 0xff,
    }
}

fn scatter_point_depth_stencil(depth_write_enabled: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: crate::DEPTH_STENCIL_FORMAT,
        depth_write_enabled: Some(depth_write_enabled),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: scatter_scene_stencil_state(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn create_scatter_point_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    label: &'static str,
    vertex_layout: wgpu::VertexBufferLayout<'static>,
    depth_write_enabled: bool,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[vertex_layout],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: Some(scatter_point_depth_stencil(depth_write_enabled)),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

impl ScatterWidget {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let points_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-points"),
            source: wgpu::ShaderSource::Wgsl(include_str!("points.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scatter-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scatter-pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = create_scatter_point_pipeline(
            device,
            surface_format,
            &pipeline_layout,
            &points_shader,
            "scatter",
            point_instance_layout(),
            false,
        );

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scatter-uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scatter-bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let fit_center = glam::Vec3::ZERO;
        let fit_radius = 5.0_f32;
        let aspect = width as f32 / height.max(1) as f32;
        let camera = Camera::fit(fit_center, fit_radius, aspect);

        Self {
            pipeline,
            opaque_pipeline: None,
            compact_pipeline: None,
            compact_xy_pipeline: None,
            compact_opaque_pipeline: None,
            clip_mask_pipeline: None,
            vertex_buffer: None,
            vertex_cap: 0,
            primary_extra_buffers: Vec::new(),
            primary_extra_caps: Vec::new(),
            primary_draw_counts: Vec::new(),
            primary_source_offsets: Vec::new(),
            primary_chunk_bounds: Vec::new(),
            primary_chunk_culling: false,
            shared_primary: None,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            point_count: 0,
            primary_points_opaque: true,
            primary_storage: PrimaryPointStorage::PointInstance,
            camera,
            offset: [0.0, 0.0],
            width,
            height,
            scissor_offset: [0, 0],
            scissor_size: [width, height],
            fit_center,
            fit_radius,
            point_size_override: -1.0,
            auto_point_size: true,
            point_size_scale: 1.0,
            interactive_render_scale: 1.0,
            auto_quality_enabled: false,
            quality_target_frame_ms: 100.0,
            quality_level: 0,
            interaction_mode: ScatterInteractionMode::Orbit3D,
            point_style: 0,
            clip_radii: [0.0; 4],
            compact_z_range: [0.0, 1.0],
            compact_colormap_len: gpu_colormap_uniform("viridis").1,
            point_color_mode: 0,
            compact_colormap: gpu_colormap_uniform("viridis").0,
            line_pipeline: None,
            line_vertex_buffer: None,
            line_vertex_cap: 0,
            grid_vertex_count: 0,
            chrome: ScatterChromeState::default(),
            grid_labels: Vec::new(),
            pending_labels: Vec::new(),
            pending_overlay_offset: 0,
            reproject_overlay_scratch: Vec::new(),
            grid_label_rects_scratch: Vec::new(),
            grid_title_projection_scratch: Vec::new(),
            last_face_bits: 0xFF,
            last_grid_bounds: None,
            grid_display_bounds: None,
            last_grid_ortho_scale: None,
            last_grid_label_projection_key: None,
            chrome_dirty: false,
            overlay_pipeline: None,
            overlay_vertex_buffer: None,
            overlay_vertex_cap: 0,
            overlay_vertex_count: 0,
            bg_pipeline: None,
            bg_vertex_buffer: None,
            bg_vertex_cap: 0,
            bg_vertex_count: 0,
            overlay_vertices_scratch: Vec::new(),
            scalar_bar_vertex_cache_key: None,
            scalar_bar_vertex_cache: Vec::new(),
            extra_actors: std::collections::HashMap::new(),
            lod_enabled: false,
            lod_threshold: 200_000,
            lod_factor: 8,
            lod_active: false,
            lod_vertex_buffer: None,
            lod_vertex_cap: 0,
            lod_sampled_scratch: Vec::new(),
            lod_bucket_keys_scratch: Vec::new(),
            lod_occupied_scratch: Vec::new(),
            picking_mode: PickingMode::Point,
            selection_rect: None,
            selection_polygon: None,
            hover_label: None,
            user_labels: Vec::new(),
            line_overlays: Vec::new(),
            user_line_vertex_buffer: None,
            user_line_vertex_cap: 0,
            user_line_vertex_count: 0,
            mesh_solid_pipeline: None,
            mesh_transparent_pipeline: None,
            mesh_wire_pipeline: None,
            mesh_actors: std::collections::HashMap::new(),
            surface_format,
            screenshot_cache: None,
            scene_revision: 1,
            last_uniforms: None,
            last_capacity_error: None,
        }
    }

    pub fn scene_revision(&self) -> u64 {
        self.scene_revision
    }

    fn mark_scene_dirty(&mut self) {
        self.scene_revision = self.scene_revision.wrapping_add(1).max(1);
    }

    fn scatter_pipeline_layout(&self, device: &wgpu::Device) -> wgpu::PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scatter-pl"),
            bind_group_layouts: &[Some(&self.bind_group_layout)],
            immediate_size: 0,
        })
    }

    fn ensure_base_render_pipelines(&mut self, device: &wgpu::Device) {
        if self.clip_mask_pipeline.is_some()
            && self.line_pipeline.is_some()
            && self.overlay_pipeline.is_some()
            && self.bg_pipeline.is_some()
        {
            return;
        }

        let pipeline_layout = self.scatter_pipeline_layout(device);
        if self.clip_mask_pipeline.is_none() {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("scatter-rounded-mask"),
                source: wgpu::ShaderSource::Wgsl(include_str!("rounded_mask.wgsl").into()),
            });
            self.clip_mask_pipeline = Some(device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("scatter-rounded-mask"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: self.surface_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::empty(),
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: crate::DEPTH_STENCIL_FORMAT,
                        depth_write_enabled: Some(false),
                        depth_compare: Some(wgpu::CompareFunction::Always),
                        stencil: scatter_clip_mask_stencil_state(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ));
        }

        if self.line_pipeline.is_none() {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("scatter-lines"),
                source: wgpu::ShaderSource::Wgsl(include_str!("lines.wgsl").into()),
            });
            self.line_pipeline = Some(device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("scatter-lines"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[line_vertex_layout()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: self.surface_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: crate::DEPTH_STENCIL_FORMAT,
                        depth_write_enabled: Some(true),
                        depth_compare: Some(wgpu::CompareFunction::Less),
                        stencil: scatter_scene_stencil_state(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ));
        }

        if self.overlay_pipeline.is_some() && self.bg_pipeline.is_some() {
            return;
        }

        let overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-overlay"),
            source: wgpu::ShaderSource::Wgsl(include_str!("overlay.wgsl").into()),
        });
        let overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("scatter-overlay-pl"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        let overlay_depth_stencil = || wgpu::DepthStencilState {
            format: crate::DEPTH_STENCIL_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: scatter_scene_stencil_state(),
            bias: wgpu::DepthBiasState::default(),
        };
        if self.overlay_pipeline.is_none() {
            self.overlay_pipeline = Some(device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("scatter-overlay"),
                    layout: Some(&overlay_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &overlay_shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[overlay_vertex_layout()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &overlay_shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: self.surface_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(overlay_depth_stencil()),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ));
        }
        if self.bg_pipeline.is_none() {
            self.bg_pipeline = Some(device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("scatter-bg"),
                    layout: Some(&overlay_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &overlay_shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[overlay_vertex_layout()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &overlay_shader,
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: self.surface_format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: Some(overlay_depth_stencil()),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                },
            ));
        }
    }

    fn ensure_opaque_point_pipeline(&mut self, device: &wgpu::Device) {
        if self.opaque_pipeline.is_some() {
            return;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-points"),
            source: wgpu::ShaderSource::Wgsl(include_str!("points.wgsl").into()),
        });
        let pipeline_layout = self.scatter_pipeline_layout(device);
        self.opaque_pipeline = Some(create_scatter_point_pipeline(
            device,
            self.surface_format,
            &pipeline_layout,
            &shader,
            "scatter-opaque",
            point_instance_layout(),
            true,
        ));
    }

    fn ensure_compact_opaque_point_pipeline(&mut self, device: &wgpu::Device) {
        if self.compact_opaque_pipeline.is_some() {
            return;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-compact-points"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compact_points.wgsl").into()),
        });
        let pipeline_layout = self.scatter_pipeline_layout(device);
        self.compact_opaque_pipeline = Some(create_scatter_point_pipeline(
            device,
            self.surface_format,
            &pipeline_layout,
            &shader,
            "scatter-compact-opaque",
            xyz_point_layout(),
            true,
        ));
    }

    fn ensure_compact_point_pipeline(&mut self, device: &wgpu::Device) {
        if self.compact_pipeline.is_some() {
            return;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-compact-points-flat"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compact_points.wgsl").into()),
        });
        let pipeline_layout = self.scatter_pipeline_layout(device);
        self.compact_pipeline = Some(create_scatter_point_pipeline(
            device,
            self.surface_format,
            &pipeline_layout,
            &shader,
            "scatter-compact-flat",
            xyz_point_layout(),
            false,
        ));
    }

    fn ensure_compact_xy_point_pipeline(&mut self, device: &wgpu::Device) {
        if self.compact_xy_pipeline.is_some() {
            return;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-compact-xy-points-flat"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compact_xy_points.wgsl").into()),
        });
        let pipeline_layout = self.scatter_pipeline_layout(device);
        self.compact_xy_pipeline = Some(create_scatter_point_pipeline(
            device,
            self.surface_format,
            &pipeline_layout,
            &shader,
            "scatter-compact-xy-flat",
            xy_point_layout(),
            false,
        ));
    }

    pub fn prepare_render_pipelines(&mut self, device: &wgpu::Device) {
        self.ensure_base_render_pipelines(device);

        let primary_is_lod =
            self.lod_enabled && self.lod_active && self.point_count > self.lod_threshold;
        let primary_visible = self.point_count > 0 && self.primary_first_buffer().is_some();
        let flat_2d = !scatter_points_write_depth(self.interaction_mode);

        // Every 3D point set needs a depth-writing pipeline, including points
        // with alpha below one. Without depth writes, visibility is determined
        // by upload order and a farther billboard can blend over a nearer one
        // after the camera rotates. Flat 2D plots intentionally retain their
        // painter-style, non-depth-writing pipelines.
        if primary_visible && !flat_2d {
            if self.primary_storage == PrimaryPointStorage::XyzF32 && !primary_is_lod {
                self.ensure_compact_opaque_point_pipeline(device);
            } else {
                self.ensure_opaque_point_pipeline(device);
            }
        }

        if primary_visible && flat_2d {
            if self.primary_storage == PrimaryPointStorage::XyF32 && !primary_is_lod {
                self.ensure_compact_xy_point_pipeline(device);
            } else if self.primary_storage == PrimaryPointStorage::XyzF32 && !primary_is_lod {
                self.ensure_compact_point_pipeline(device);
            }
        }

        if !flat_2d
            && self.extra_actors.values().any(|actor| {
                actor.visible && actor.point_count > 0 && actor.vertex_buffer.is_some()
            })
        {
            self.ensure_opaque_point_pipeline(device);
        }
    }

    fn set_single_primary_draw(&mut self, point_count: u32) {
        self.shared_primary = None;
        self.primary_extra_buffers.clear();
        self.primary_extra_caps.clear();
        self.primary_draw_counts.clear();
        self.primary_source_offsets.clear();
        self.primary_chunk_bounds.clear();
        self.primary_chunk_culling = false;
        if point_count > 0 {
            self.primary_draw_counts.push(point_count);
            self.primary_source_offsets.push(0);
            self.primary_chunk_bounds.push(None);
        }
    }

    fn clear_primary_draw_buffers(&mut self) {
        self.shared_primary = None;
        self.vertex_buffer = None;
        self.vertex_cap = 0;
        self.primary_extra_buffers.clear();
        self.primary_extra_caps.clear();
        self.primary_draw_counts.clear();
        self.primary_source_offsets.clear();
        self.primary_chunk_bounds.clear();
        self.primary_chunk_culling = false;
    }

    pub fn primary_chunk_count(&self) -> usize {
        self.primary_draw_counts.len()
    }

    pub fn primary_chunk_source_offsets(&self) -> &[u32] {
        &self.primary_source_offsets
    }

    fn primary_chunk_visible_with_view_proj(&self, index: usize, view_proj: glam::Mat4) -> bool {
        if !self.primary_chunk_culling {
            return true;
        }
        self.primary_chunk_bounds
            .get(index)
            .copied()
            .flatten()
            .is_some_and(|bounds| aabb_intersects_clip(bounds, view_proj))
    }

    pub fn visible_primary_chunk_count(&self) -> usize {
        let view_proj = self.camera.view_proj();
        (0..self.primary_chunk_count())
            .filter(|index| self.primary_chunk_visible_with_view_proj(*index, view_proj))
            .count()
    }

    pub fn visible_primary_candidate_rows(&self) -> u64 {
        let view_proj = self.camera.view_proj();
        self.primary_draw_counts
            .iter()
            .enumerate()
            .filter(|(index, _)| self.primary_chunk_visible_with_view_proj(*index, view_proj))
            .map(|(_, count)| *count as u64)
            .sum()
    }

    fn primary_first_buffer(&self) -> Option<&wgpu::Buffer> {
        self.shared_primary
            .as_ref()
            .and_then(|shared| shared.buffers.first())
            .or(self.vertex_buffer.as_ref())
    }

    pub(crate) fn promote_primary_to_shared(&mut self) -> Option<Arc<SharedPointBuffers>> {
        if let Some(shared) = &self.shared_primary {
            return Some(shared.clone());
        }
        if self.point_count == 0 {
            return None;
        }
        let first = self.vertex_buffer.take()?;
        let mut buffers = Vec::with_capacity(1 + self.primary_extra_buffers.len());
        buffers.push(first);
        buffers.append(&mut self.primary_extra_buffers);
        let mut capacities = Vec::with_capacity(1 + self.primary_extra_caps.len());
        capacities.push(std::mem::take(&mut self.vertex_cap));
        capacities.append(&mut self.primary_extra_caps);
        let shared = Arc::new(SharedPointBuffers {
            buffers,
            capacities,
            draw_counts: self.primary_draw_counts.clone(),
            source_offsets: self.primary_source_offsets.clone(),
            chunk_bounds: self.primary_chunk_bounds.clone(),
            chunk_culling: self.primary_chunk_culling,
            point_count: self.point_count,
            storage: self.primary_storage,
            points_opaque: self.primary_points_opaque,
        });
        self.shared_primary = Some(shared.clone());
        Some(shared)
    }

    pub(crate) fn set_shared_buffers(
        &mut self,
        shared: Arc<SharedPointBuffers>,
        data_min: glam::Vec3,
        data_max: glam::Vec3,
        colormap: &str,
        queue: &wgpu::Queue,
    ) -> ScatterUploadTimings {
        self.point_color_mode = 0;
        self.vertex_buffer = None;
        self.vertex_cap = 0;
        self.primary_extra_buffers.clear();
        self.primary_extra_caps.clear();
        self.primary_draw_counts = shared.draw_counts.clone();
        self.primary_source_offsets = shared.source_offsets.clone();
        self.primary_chunk_bounds = shared.chunk_bounds.clone();
        self.primary_chunk_culling = shared.chunk_culling;
        self.point_count = shared.point_count;
        self.primary_storage = shared.storage;
        self.primary_points_opaque = shared.points_opaque;
        self.shared_primary = Some(shared);
        if self.primary_storage == PrimaryPointStorage::XyzF32 {
            let (gpu_colormap, gpu_colormap_len) = gpu_colormap_uniform(colormap);
            self.compact_colormap = gpu_colormap;
            self.compact_colormap_len = gpu_colormap_len;
            self.compact_z_range = [data_min.z, data_max.z];
        }
        self.lod_vertex_buffer = None;
        self.lod_vertex_cap = 0;
        self.last_capacity_error = None;
        self.mark_scene_dirty();
        self.recompute_point_size_scale();
        self.update_camera(queue);
        ScatterUploadTimings::default()
    }

    /// Select presentation-neutral density coloring for PointInstance buffers.
    /// Mode 1 interprets red as normalized intensity and applies the per-widget
    /// colormap in the shader, so multiple plots can share the same geometry.
    pub(crate) fn set_density_colormap_mode(
        &mut self,
        enabled: bool,
        colormap: &str,
        queue: &wgpu::Queue,
    ) {
        self.point_color_mode = u32::from(enabled);
        if enabled {
            let (gpu_colormap, gpu_colormap_len) = gpu_colormap_uniform(colormap);
            self.compact_colormap = gpu_colormap;
            self.compact_colormap_len = gpu_colormap_len;
        }
        self.update_camera(queue);
    }

    /// Upload point data to GPU.  Reallocates the vertex buffer if needed.
    pub fn set_points(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        points: &[PointInstance],
    ) -> ScatterUploadTimings {
        self.point_color_mode = 0;
        let size = (points.len() * std::mem::size_of::<PointInstance>()) as u64;
        if size == 0 {
            self.last_capacity_error = None;
            self.point_count = 0;
            self.set_single_primary_draw(0);
            self.primary_points_opaque = true;
            self.primary_storage = PrimaryPointStorage::PointInstance;
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
            self.mark_scene_dirty();
            self.recompute_point_size_scale();
            self.update_camera(queue);
            return ScatterUploadTimings::default();
        }
        if self.vertex_buffer.is_none() || size > self.vertex_cap {
            // Over-allocate by 2× so incremental updates don't thrash.
            let Some(cap) = bounded_scatter_buffer_capacity(size, device.limits().max_buffer_size)
            else {
                self.last_capacity_error = Some(scatter_capacity_error(
                    size,
                    device.limits().max_buffer_size,
                    std::mem::size_of::<PointInstance>() as u64,
                    "point_instance_v1",
                ));
                self.clear_primary_draw_buffers();
                self.point_count = 0;
                self.lod_vertex_buffer = None;
                self.lod_vertex_cap = 0;
                self.mark_scene_dirty();
                return ScatterUploadTimings::default();
            };
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_cap = cap;
        }
        self.last_capacity_error = None;
        let primary_t0 = Instant::now();
        queue.write_buffer(
            self.vertex_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(points),
        );
        let primary_ms = primary_t0.elapsed().as_secs_f64() * 1000.0;
        self.point_count = points.len() as u32;
        self.set_single_primary_draw(self.point_count);
        self.primary_points_opaque = points_are_opaque(points);
        self.primary_storage = PrimaryPointStorage::PointInstance;
        self.mark_scene_dirty();
        let lod_ms = if self.should_build_active_lod(self.point_count) {
            let lod_t0 = Instant::now();
            upload_lod_buffer(
                points,
                self.lod_factor,
                &mut self.lod_vertex_buffer,
                &mut self.lod_vertex_cap,
                device,
                queue,
                &mut self.lod_sampled_scratch,
                &mut self.lod_bucket_keys_scratch,
                &mut self.lod_occupied_scratch,
            );
            lod_t0.elapsed().as_secs_f64() * 1000.0
        } else {
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
            0.0
        };
        self.recompute_point_size_scale();
        self.update_camera(queue);
        ScatterUploadTimings { primary_ms, lod_ms }
    }

    /// Upload a derived point product and release a materially oversized exact buffer.
    /// Density/representative products are disposable, so retaining an exact-sized
    /// allocation after a policy toggle defeats their GPU-memory benefit.
    pub fn set_derived_points(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        points: &[PointInstance],
    ) -> ScatterUploadTimings {
        let required = (points.len() * std::mem::size_of::<PointInstance>()) as u64;
        let target = bounded_scatter_buffer_capacity(required, device.limits().max_buffer_size)
            .unwrap_or(required);
        if self.vertex_cap > target {
            self.vertex_buffer = None;
            self.vertex_cap = 0;
        }
        self.set_points(device, queue, points)
    }

    /// Upload an already GPU-shaped point_instance_v1 payload directly.
    ///
    /// This avoids rebuilding a CPU `Vec<PointInstance>` for high-rate full-frame
    /// streams. It intentionally declines the fast path while interaction LOD is
    /// active because LOD sampling currently needs the CPU point slice.
    pub fn set_point_instances_raw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> Result<Option<ScatterUploadTimings>, ScatterCapacityError> {
        const STRIDE: usize = std::mem::size_of::<PointInstance>();
        if bytes.len() % STRIDE != 0 {
            return Ok(None);
        }
        let point_count = bytes.len() / STRIDE;
        if self.should_build_active_lod(point_count as u32) {
            return Ok(None);
        }
        self.point_color_mode = 0;
        self.shared_primary = None;
        let size = bytes.len() as u64;
        if size == 0 {
            self.last_capacity_error = None;
            self.point_count = 0;
            self.set_single_primary_draw(0);
            self.primary_points_opaque = true;
            self.primary_storage = PrimaryPointStorage::PointInstance;
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
            self.mark_scene_dirty();
            self.recompute_point_size_scale();
            self.update_camera(queue);
            return Ok(Some(ScatterUploadTimings::default()));
        }
        if self.vertex_buffer.is_none() || size > self.vertex_cap {
            let Some(cap) = bounded_scatter_buffer_capacity(size, device.limits().max_buffer_size)
            else {
                let error = scatter_capacity_error(
                    size,
                    device.limits().max_buffer_size,
                    STRIDE as u64,
                    "point_instance_v1",
                );
                self.last_capacity_error = Some(error.clone());
                self.clear_primary_draw_buffers();
                self.point_count = 0;
                self.lod_vertex_buffer = None;
                self.lod_vertex_cap = 0;
                self.mark_scene_dirty();
                return Err(error);
            };
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_cap = cap;
        }
        self.last_capacity_error = None;
        let primary_t0 = Instant::now();
        queue.write_buffer(self.vertex_buffer.as_ref().unwrap(), 0, bytes);
        let primary_ms = primary_t0.elapsed().as_secs_f64() * 1000.0;
        self.point_count = point_count as u32;
        self.set_single_primary_draw(self.point_count);
        self.primary_points_opaque = point_instance_bytes_are_opaque(bytes);
        self.primary_storage = PrimaryPointStorage::PointInstance;
        self.lod_vertex_buffer = None;
        self.lod_vertex_cap = 0;
        self.mark_scene_dirty();
        self.recompute_point_size_scale();
        self.update_camera(queue);
        Ok(Some(ScatterUploadTimings {
            primary_ms,
            lod_ms: 0.0,
        }))
    }

    /// Upload a compact xy_f32_v0 payload directly for a flat 2D scatter.
    pub fn set_xy_points_raw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
    ) -> Result<Option<ScatterUploadTimings>, ScatterCapacityError> {
        if bytes.len() % 8 != 0 || self.interaction_mode != ScatterInteractionMode::Pan2D {
            return Ok(None);
        }
        let point_count = bytes.len() / 8;
        if self.should_build_active_lod(point_count as u32) {
            return Ok(None);
        }
        let size = bytes.len() as u64;
        if size == 0 {
            self.last_capacity_error = None;
            self.point_count = 0;
            self.set_single_primary_draw(0);
            self.primary_points_opaque = true;
            self.primary_storage = PrimaryPointStorage::XyF32;
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
            self.mark_scene_dirty();
            self.recompute_point_size_scale();
            self.update_camera(queue);
            return Ok(Some(ScatterUploadTimings::default()));
        }
        let max_buffer_size = device.limits().max_buffer_size;
        let Some((chunk_bytes, chunk_count)) = scatter_chunk_layout(size, max_buffer_size, 8)
        else {
            let error = scatter_capacity_error(size, max_buffer_size, 8, "chunked_xy_f32_v0");
            self.last_capacity_error = Some(error.clone());
            self.clear_primary_draw_buffers();
            self.point_count = 0;
            self.mark_scene_dirty();
            return Err(error);
        };
        let first_size = size.min(chunk_bytes);
        if self.vertex_buffer.is_none() || first_size > self.vertex_cap {
            let cap = if chunk_count == 1 {
                bounded_scatter_buffer_capacity(first_size, max_buffer_size).unwrap()
            } else {
                first_size
            };
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-compact-xy-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_cap = cap;
        }

        let extra_count = chunk_count.saturating_sub(1);
        while self.primary_extra_buffers.len() < extra_count {
            let index = self.primary_extra_buffers.len() + 1;
            let offset = index as u64 * chunk_bytes;
            let required = (size - offset).min(chunk_bytes);
            let cap = bounded_scatter_buffer_capacity(required, max_buffer_size).unwrap();
            self.primary_extra_buffers
                .push(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("scatter-compact-xy-chunk-vb"),
                    size: cap,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.primary_extra_caps.push(cap);
        }
        for index in 0..extra_count {
            let offset = (index as u64 + 1) * chunk_bytes;
            let required = (size - offset).min(chunk_bytes);
            if required > self.primary_extra_caps[index] {
                let cap = bounded_scatter_buffer_capacity(required, max_buffer_size).unwrap();
                self.primary_extra_buffers[index] = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("scatter-compact-xy-chunk-vb"),
                    size: cap,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.primary_extra_caps[index] = cap;
            }
        }
        self.primary_extra_buffers.truncate(extra_count);
        self.primary_extra_caps.truncate(extra_count);

        self.last_capacity_error = None;
        let primary_t0 = Instant::now();
        let chunk_bytes_usize = chunk_bytes as usize;
        for (index, chunk) in bytes.chunks(chunk_bytes_usize).enumerate() {
            let buffer = if index == 0 {
                self.vertex_buffer.as_ref().unwrap()
            } else {
                &self.primary_extra_buffers[index - 1]
            };
            queue.write_buffer(buffer, 0, chunk);
        }
        let primary_ms = primary_t0.elapsed().as_secs_f64() * 1000.0;
        self.point_count = point_count as u32;
        self.primary_draw_counts = bytes
            .chunks(chunk_bytes_usize)
            .map(|chunk| (chunk.len() / 8) as u32)
            .collect();
        self.primary_source_offsets = self
            .primary_draw_counts
            .iter()
            .scan(0_u32, |offset, count| {
                let current = *offset;
                *offset = offset.saturating_add(*count);
                Some(current)
            })
            .collect();
        self.primary_chunk_bounds = vec![None; self.primary_draw_counts.len()];
        self.primary_chunk_culling = false;
        self.primary_points_opaque = true;
        self.primary_storage = PrimaryPointStorage::XyF32;
        self.lod_vertex_buffer = None;
        self.lod_vertex_cap = 0;
        self.mark_scene_dirty();
        self.recompute_point_size_scale();
        self.update_camera(queue);
        Ok(Some(ScatterUploadTimings {
            primary_ms,
            lod_ms: 0.0,
        }))
    }

    /// Upload a compact xyz_f32_v0 payload directly and color it in the point shader.
    ///
    /// The path is limited to opaque default-size z-colormapped points. It declines
    /// while active interaction LOD needs a sampled full PointInstance buffer.
    pub fn set_xyz_points_raw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        data_min: glam::Vec3,
        data_max: glam::Vec3,
        colormap: &str,
        source_chunk_rows: Option<usize>,
    ) -> Result<Option<ScatterUploadTimings>, ScatterCapacityError> {
        if bytes.len() % 12 != 0 || !data_min.is_finite() || !data_max.is_finite() {
            return Ok(None);
        }
        self.shared_primary = None;
        let point_count = bytes.len() / 12;
        if self.should_build_active_lod(point_count as u32) {
            return Ok(None);
        }
        let size = bytes.len() as u64;
        if size == 0 {
            self.last_capacity_error = None;
            self.point_count = 0;
            self.set_single_primary_draw(0);
            self.primary_points_opaque = true;
            self.primary_storage = PrimaryPointStorage::XyzF32;
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
            self.mark_scene_dirty();
            self.recompute_point_size_scale();
            self.update_camera(queue);
            return Ok(Some(ScatterUploadTimings::default()));
        }
        let max_buffer_size = device.limits().max_buffer_size;
        let requested_chunk_bytes = source_chunk_rows
            .and_then(|rows| rows.checked_mul(12))
            .map(|bytes| bytes as u64)
            .unwrap_or(max_buffer_size)
            .min(max_buffer_size);
        let Some((chunk_bytes, chunk_count)) =
            scatter_chunk_layout(size, requested_chunk_bytes, 12)
        else {
            let error = scatter_capacity_error(size, max_buffer_size, 12, "chunked_xyz_f32_v0");
            self.last_capacity_error = Some(error.clone());
            self.clear_primary_draw_buffers();
            self.point_count = 0;
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
            self.mark_scene_dirty();
            return Err(error);
        };
        let first_size = size.min(chunk_bytes);
        if self.vertex_buffer.is_none() || first_size > self.vertex_cap {
            let cap = if chunk_count == 1 && source_chunk_rows.is_none() {
                bounded_scatter_buffer_capacity(first_size, max_buffer_size).unwrap()
            } else {
                first_size
            };
            self.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-compact-xyz-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.vertex_cap = cap;
        }
        let extra_count = chunk_count.saturating_sub(1);
        while self.primary_extra_buffers.len() < extra_count {
            let index = self.primary_extra_buffers.len() + 1;
            let offset = index as u64 * chunk_bytes;
            let required = (size - offset).min(chunk_bytes);
            let cap = required;
            self.primary_extra_buffers
                .push(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("scatter-compact-xyz-chunk-vb"),
                    size: cap,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.primary_extra_caps.push(cap);
        }
        for index in 0..extra_count {
            let offset = (index as u64 + 1) * chunk_bytes;
            let required = (size - offset).min(chunk_bytes);
            if required > self.primary_extra_caps[index] {
                let cap = required;
                self.primary_extra_buffers[index] = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("scatter-compact-xyz-chunk-vb"),
                    size: cap,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.primary_extra_caps[index] = cap;
            }
        }
        self.primary_extra_buffers.truncate(extra_count);
        self.primary_extra_caps.truncate(extra_count);

        self.last_capacity_error = None;
        let primary_t0 = Instant::now();
        let chunk_bytes_usize = chunk_bytes as usize;
        for (index, chunk) in bytes.chunks(chunk_bytes_usize).enumerate() {
            let buffer = if index == 0 {
                self.vertex_buffer.as_ref().unwrap()
            } else {
                &self.primary_extra_buffers[index - 1]
            };
            queue.write_buffer(buffer, 0, chunk);
        }
        let primary_ms = primary_t0.elapsed().as_secs_f64() * 1000.0;
        let (gpu_colormap, gpu_colormap_len) = gpu_colormap_uniform(colormap);
        self.compact_colormap = gpu_colormap;
        self.compact_colormap_len = gpu_colormap_len;
        self.compact_z_range = [data_min.z, data_max.z];
        self.point_count = point_count as u32;
        self.primary_draw_counts = bytes
            .chunks(chunk_bytes_usize)
            .map(|chunk| (chunk.len() / 12) as u32)
            .collect();
        self.primary_source_offsets = self
            .primary_draw_counts
            .iter()
            .scan(0_u32, |offset, count| {
                let current = *offset;
                *offset = offset.saturating_add(*count);
                Some(current)
            })
            .collect();
        self.primary_chunk_bounds = bytes
            .chunks(chunk_bytes_usize)
            .map(compact_xyz_chunk_bounds)
            .collect();
        self.primary_chunk_culling = true;
        self.primary_points_opaque = true;
        self.primary_storage = PrimaryPointStorage::XyzF32;
        self.lod_vertex_buffer = None;
        self.lod_vertex_cap = 0;
        self.mark_scene_dirty();
        self.recompute_point_size_scale();
        self.update_camera(queue);
        Ok(Some(ScatterUploadTimings {
            primary_ms,
            lod_ms: 0.0,
        }))
    }

    pub fn last_capacity_error(&self) -> Option<&ScatterCapacityError> {
        self.last_capacity_error.as_ref()
    }

    fn should_build_lod(&self, point_count: u32) -> bool {
        self.lod_enabled && point_count > self.lod_threshold
    }

    fn should_build_active_lod(&self, point_count: u32) -> bool {
        self.lod_active && self.should_build_lod(point_count)
    }

    pub fn set_compact_colormap(&mut self, colormap: &str, queue: &wgpu::Queue) -> bool {
        if !matches!(
            self.primary_storage,
            PrimaryPointStorage::XyF32 | PrimaryPointStorage::XyzF32
        ) {
            return false;
        }
        let (gpu_colormap, gpu_colormap_len) = gpu_colormap_uniform(colormap);
        self.compact_colormap = gpu_colormap;
        self.compact_colormap_len = gpu_colormap_len;
        self.update_camera(queue);
        true
    }

    pub fn refresh_lod_buffers(
        &mut self,
        primary_points: &[PointInstance],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> f64 {
        let t0 = Instant::now();
        if self.should_build_lod(self.point_count) {
            upload_lod_buffer(
                primary_points,
                self.lod_factor,
                &mut self.lod_vertex_buffer,
                &mut self.lod_vertex_cap,
                device,
                queue,
                &mut self.lod_sampled_scratch,
                &mut self.lod_bucket_keys_scratch,
                &mut self.lod_occupied_scratch,
            );
        } else {
            self.lod_vertex_buffer = None;
            self.lod_vertex_cap = 0;
        }

        let lod_enabled = self.lod_enabled;
        let lod_threshold = self.lod_threshold;
        let lod_factor = self.lod_factor;
        for actor in self.extra_actors.values_mut() {
            let build_lod =
                lod_enabled && actor.stream_mode.is_none() && actor.point_count > lod_threshold;
            if build_lod {
                upload_lod_buffer(
                    &actor.points,
                    lod_factor,
                    &mut actor.lod_vertex_buffer,
                    &mut actor.lod_vertex_cap,
                    device,
                    queue,
                    &mut actor.lod_sampled_scratch,
                    &mut actor.lod_bucket_keys_scratch,
                    &mut actor.lod_occupied_scratch,
                );
            } else if actor.stream_mode.is_none() {
                actor.lod_vertex_buffer = None;
                actor.lod_vertex_cap = 0;
            }
        }
        self.recompute_point_size_scale();
        self.update_camera(queue);
        t0.elapsed().as_secs_f64() * 1000.0
    }

    /// Write current camera state into the uniform buffer.
    pub fn update_camera(&mut self, queue: &wgpu::Queue) {
        self.recompute_point_size_scale();
        let uniforms = self.uniforms_for_size(self.width, self.height);
        if self.last_uniforms != Some(uniforms) {
            queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
            self.last_uniforms = Some(uniforms);
            self.mark_scene_dirty();
        }
    }

    fn uniforms_for_size(&self, width: u32, height: u32) -> Uniforms {
        Uniforms {
            view_proj: self.camera.view_proj().to_cols_array_2d(),
            screen_size: [width as f32, height as f32],
            style: self.point_style,
            point_size: self.point_size_override,
            point_size_scale: self.point_size_scale,
            _pad0: [0.0; 3],
            clip_radii: self.clip_radii,
            compact_z_range: self.compact_z_range,
            compact_colormap_len: self.compact_colormap_len,
            point_color_mode: self.point_color_mode,
            compact_colormap: self.compact_colormap,
        }
    }

    fn recompute_point_size_scale(&mut self) {
        self.point_size_scale = if self.auto_point_size {
            let density_scale = adaptive_point_size_scale(
                self.effective_draw_point_count(),
                self.width,
                self.height,
            );
            let fit_distance =
                fitted_camera_distance(self.fit_radius, self.camera.aspect, self.camera.fov_y);
            density_scale * zoom_out_point_size_scale(self.camera.distance, fit_distance)
        } else {
            1.0
        };
    }

    pub fn set_auto_point_size(&mut self, enabled: bool, queue: &wgpu::Queue) {
        self.auto_point_size = enabled;
        self.recompute_point_size_scale();
        self.update_camera(queue);
    }

    pub fn set_interactive_render_scale(&mut self, scale: f32) {
        self.interactive_render_scale = clamp_interactive_render_scale(scale);
    }

    pub fn set_auto_quality(&mut self, enabled: bool, target_frame_ms: f32) {
        self.auto_quality_enabled = enabled;
        if target_frame_ms.is_finite() && target_frame_ms > 0.0 {
            self.quality_target_frame_ms = target_frame_ms.max(4.0);
        }
        if !enabled {
            self.quality_level = 0;
        }
    }

    pub fn set_quality_level(&mut self, level: u32) {
        self.quality_level = level.min(3);
    }

    pub fn active_render_scale(&self) -> f32 {
        if self.lod_active {
            if self.auto_quality_enabled {
                let budget_scale = match self.quality_level {
                    0 => 1.0,
                    1 => 0.75,
                    2 => 0.55,
                    _ => 0.40,
                };
                self.interactive_render_scale.min(budget_scale)
            } else {
                self.interactive_render_scale
            }
        } else {
            1.0
        }
    }

    pub fn refresh_point_size_scale(&mut self, queue: &wgpu::Queue) {
        self.recompute_point_size_scale();
        self.update_camera(queue);
    }

    pub fn set_lod_active(&mut self, active: bool, queue: &wgpu::Queue) {
        self.lod_active = active;
        self.recompute_point_size_scale();
        self.update_camera(queue);
    }

    pub fn set_point_size_override(&mut self, point_size: Option<f32>, queue: &wgpu::Queue) {
        self.point_size_override = point_size_override_value(point_size);
        self.recompute_point_size_scale();
        self.update_camera(queue);
    }

    pub fn set_point_style(&mut self, style: Option<&str>, queue: &wgpu::Queue) {
        self.point_style = match style {
            Some("square") => 1,
            Some("gaussian") => 2,
            _ => 0, // circle is the default
        };
        self.update_camera(queue);
    }

    fn effective_point_size(&self, base_size: f32) -> f32 {
        let size = if self.point_size_override >= 0.0 {
            self.point_size_override
        } else {
            base_size
        };
        size * self.point_size_scale
    }

    pub fn effective_draw_point_count(&self) -> u32 {
        let primary =
            if self.lod_enabled && self.lod_active && self.point_count > self.lod_threshold {
                lod_sample_count(self.point_count as usize, self.lod_factor) as u32
            } else {
                self.point_count
            };
        primary
            + self
                .extra_actors
                .values()
                .filter(|actor| actor.visible && actor.point_count > 0)
                .map(|actor| {
                    if self.lod_enabled && self.lod_active && actor.point_count > self.lod_threshold
                    {
                        lod_sample_count(actor.point_count as usize, self.lod_factor) as u32
                    } else {
                        actor.point_count
                    }
                })
                .sum::<u32>()
    }

    pub fn primary_representation(&self) -> &'static str {
        if self.point_count == 0 || self.primary_first_buffer().is_none() {
            "empty"
        } else if self.lod_enabled && self.lod_active && self.point_count > self.lod_threshold {
            "lod_point_instance"
        } else {
            match self.primary_storage {
                PrimaryPointStorage::PointInstance => "point_instance_v1",
                PrimaryPointStorage::XyF32 if self.primary_chunk_count() > 1 => "chunked_xy_f32_v0",
                PrimaryPointStorage::XyF32 => "compact_xy_f32_v0",
                PrimaryPointStorage::XyzF32 if self.primary_chunk_count() > 1 => {
                    "chunked_xyz_f32_v0"
                }
                PrimaryPointStorage::XyzF32 => "compact_xyz_f32_v0",
            }
        }
    }

    pub fn representation_reason(&self) -> &'static str {
        if self.point_count == 0 || self.primary_first_buffer().is_none() {
            "no_renderable_primary_buffer"
        } else if self.lod_enabled && self.lod_active && self.point_count > self.lod_threshold {
            "interaction_lod_active_above_threshold"
        } else {
            match self.primary_storage {
                PrimaryPointStorage::PointInstance => "payload_requires_per_point_style",
                PrimaryPointStorage::XyF32 if self.primary_chunk_count() > 1 => {
                    "2d_exact_payload_split_at_device_buffer_limit"
                }
                PrimaryPointStorage::XyF32 => "2d_position_only_payload_uses_compact_layout",
                PrimaryPointStorage::XyzF32
                    if self.primary_chunk_count() > 1
                        && self.primary_chunk_bounds.iter().any(Option::is_some) =>
                {
                    "3d_point_store_source_chunks_enable_frustum_culling"
                }
                PrimaryPointStorage::XyzF32 if self.primary_chunk_count() > 1 => {
                    "3d_exact_payload_split_at_device_buffer_limit"
                }
                PrimaryPointStorage::XyzF32 => "position_only_payload_uses_compact_layout",
            }
        }
    }

    pub fn gpu_memory_stats(&self) -> ScatterGpuMemoryStats {
        let primary_allocated_bytes = self.shared_primary.as_ref().map_or_else(
            || {
                self.vertex_buffer
                    .as_ref()
                    .map_or(0, |_| self.vertex_cap)
                    .saturating_add(self.primary_extra_caps.iter().copied().sum::<u64>())
            },
            |shared| shared.allocated_bytes(),
        );
        let primary_stride = match self.primary_storage {
            PrimaryPointStorage::PointInstance => std::mem::size_of::<PointInstance>() as u64,
            PrimaryPointStorage::XyF32 => 8,
            PrimaryPointStorage::XyzF32 => 12,
        };
        let primary_used_bytes = self
            .primary_first_buffer()
            .map_or(0, |_| self.point_count as u64 * primary_stride);
        let lod_allocated_bytes = self
            .lod_vertex_buffer
            .as_ref()
            .map_or(0, |_| self.lod_vertex_cap);
        let lod_used_bytes = self.lod_vertex_buffer.as_ref().map_or(0, |_| {
            lod_sample_count(self.point_count as usize, self.lod_factor) as u64
                * std::mem::size_of::<PointInstance>() as u64
        });
        let actor_allocated_bytes = self.extra_actors.values().fold(0_u64, |total, actor| {
            total
                .saturating_add(actor.vertex_buffer.as_ref().map_or(0, |_| actor.vertex_cap))
                .saturating_add(
                    actor
                        .lod_vertex_buffer
                        .as_ref()
                        .map_or(0, |_| actor.lod_vertex_cap),
                )
        });
        let chrome_allocated_bytes = self
            .line_vertex_buffer
            .as_ref()
            .map_or(0, |_| self.line_vertex_cap)
            .saturating_add(
                self.overlay_vertex_buffer
                    .as_ref()
                    .map_or(0, |_| self.overlay_vertex_cap),
            )
            .saturating_add(
                self.bg_vertex_buffer
                    .as_ref()
                    .map_or(0, |_| self.bg_vertex_cap),
            )
            .saturating_add(
                self.user_line_vertex_buffer
                    .as_ref()
                    .map_or(0, |_| self.user_line_vertex_cap),
            );
        let mesh_allocated_bytes = self.mesh_actors.values().fold(0_u64, |total, actor| {
            let vertex_bytes = actor.vertex_buffer.as_ref().map_or(0, |_| {
                actor.positions.len() as u64 * std::mem::size_of::<MeshVertex>() as u64
            });
            let index_bytes = actor.index_buffer.as_ref().map_or(0, |_| {
                actor.index_count as u64 * std::mem::size_of::<u32>() as u64
            });
            total
                .saturating_add(vertex_bytes)
                .saturating_add(index_bytes)
        });
        let uniform_allocated_bytes = std::mem::size_of::<Uniforms>() as u64;
        let total_allocated_bytes = primary_allocated_bytes
            .saturating_add(lod_allocated_bytes)
            .saturating_add(actor_allocated_bytes)
            .saturating_add(chrome_allocated_bytes)
            .saturating_add(mesh_allocated_bytes)
            .saturating_add(uniform_allocated_bytes);
        ScatterGpuMemoryStats {
            primary_allocated_bytes,
            primary_used_bytes,
            primary_shared: self.shared_primary.is_some(),
            lod_allocated_bytes,
            lod_used_bytes,
            actor_allocated_bytes,
            chrome_allocated_bytes,
            mesh_allocated_bytes,
            uniform_allocated_bytes,
            total_allocated_bytes,
        }
    }

    /// Place the scatter inside a sub-region of the window.
    ///
    /// Updates the stored offset, dimensions, camera aspect ratio, and
    /// uniform buffer.  Call this after every layout recomputation.
    pub fn set_layout_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        visible_clip: Option<[f32; 4]>,
        clip_radii: [f32; 4],
        queue: &wgpu::Queue,
    ) {
        let rect = scatter_layout_rect(x, y, w, h, visible_clip);
        let dims_changed = self.width != rect.width || self.height != rect.height;
        let clip_radii = clamp_clip_radii(clip_radii, w, h);
        let clip_changed = self.clip_radii != clip_radii;
        self.offset = rect.offset;
        self.width = rect.width;
        self.height = rect.height;
        self.scissor_offset = rect.scissor_offset;
        self.scissor_size = rect.scissor_size;
        self.clip_radii = clip_radii;
        self.camera.aspect = w / h.max(1.0);
        self.recompute_point_size_scale();
        // Screen position and scissor changes happen continuously while a parent
        // scrolls. They do not change the scatter scene itself, so keep the
        // cached render target valid and let the compositor apply the new clip.
        if dims_changed || clip_changed {
            self.mark_scene_dirty();
        }
        self.update_camera(queue);
        if dims_changed {
            self.screenshot_cache = None;
        }
    }

    /// Returns the visible scissor bounds as [left, top, right, bottom] in physical pixels.
    ///
    /// Overlay text is rendered by the window-level text compositor rather than
    /// the scatter render pass, so it must receive the same scroll-aware clip as
    /// the scatter geometry. Returning the full viewport here lets scalar-bar and
    /// axis labels paint through a clipped scroll-panel boundary.
    pub fn viewport_clip(&self) -> [f32; 4] {
        scatter_scissor_bounds(self.scissor_offset, self.scissor_size)
    }

    /// True when the scatter has a non-empty visible viewport.
    ///
    /// Hidden scatters can still receive data updates while their owning page
    /// is inactive.  Camera fitting must wait until the widget has real
    /// dimensions, otherwise a zero-width layout produces an extreme aspect
    /// ratio and an unusably distant camera.
    pub fn has_visible_viewport(&self) -> bool {
        self.width > 1 && self.height > 1 && self.scissor_size[0] > 1 && self.scissor_size[1] > 1
    }

    /// Visible XY data bounds for the flat orthographic interaction mode.
    pub fn visible_xy_bounds(&self) -> Option<[f32; 4]> {
        if self.interaction_mode != ScatterInteractionMode::Pan2D || !self.camera.parallel {
            return None;
        }
        let (half_w, half_h) = if self.camera.ortho_half_w > 0.0 && self.camera.ortho_half_h > 0.0 {
            (self.camera.ortho_half_w, self.camera.ortho_half_h)
        } else {
            let half_h = self.camera.distance * (self.camera.fov_y * 0.5).tan();
            (half_h * self.camera.aspect, half_h)
        };
        Some([
            self.camera.target.x - half_w,
            self.camera.target.y - half_h,
            self.camera.target.x + half_w,
            self.camera.target.y + half_h,
        ])
    }

    /// Zoom while preserving the 2D data coordinate beneath the pointer.
    ///
    /// Three-dimensional scatters retain their center-zoom behavior. Flat
    /// orthographic plots translate the camera target after changing scale so
    /// repeated wheel input can drill into an off-center feature.
    pub fn zoom_at_position(&mut self, window_position: [f32; 2], delta: f32) {
        let pointer_fraction = if self.interaction_mode == ScatterInteractionMode::Pan2D {
            Some([
                ((window_position[0] - self.offset[0]) / self.width.max(1) as f32).clamp(0.0, 1.0),
                ((window_position[1] - self.offset[1]) / self.height.max(1) as f32).clamp(0.0, 1.0),
            ])
        } else {
            None
        };
        let before = pointer_fraction.and_then(|_| self.visible_xy_bounds());
        self.camera.zoom(delta);
        if let (Some(fraction), Some(before), Some(after)) =
            (pointer_fraction, before, self.visible_xy_bounds())
        {
            let adjustment = pan2d_cursor_anchor_adjustment(before, after, fraction);
            self.camera.target.x += adjustment.x;
            self.camera.target.y += adjustment.y;
        }
    }

    /// Restore the camera to its initial fit position (R / Home key).
    pub fn reset_camera(&mut self, queue: &wgpu::Queue) {
        let aspect = self.width as f32 / self.height.max(1) as f32;
        self.camera = Camera::fit(self.fit_center, self.fit_radius, aspect);
        self.update_camera(queue);
    }

    /// Set a preset view direction without changing target/distance.
    /// Accepts "xy" (from +Z), "xz" (from +Y), "yz" (from +X), "isometric".
    pub fn set_view_direction(&mut self, direction: &str, queue: &wgpu::Queue) {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
        match direction {
            "xy" => {
                self.camera.pitch = 0.0;
                self.camera.yaw = 0.0;
            }
            "xz" => {
                self.camera.pitch = FRAC_PI_2;
                self.camera.yaw = 0.0;
            }
            "yz" => {
                self.camera.pitch = 0.0;
                self.camera.yaw = FRAC_PI_2;
            }
            "isometric" => {
                self.camera.pitch = 0.6155_f32; // arctan(1/√2) ≈ 35.26°
                self.camera.yaw = FRAC_PI_4;
            }
            _ => return,
        }
        self.update_camera(queue);
    }

    pub fn set_parallel_projection(&mut self, parallel: bool, queue: &wgpu::Queue) {
        self.camera.parallel = parallel;
        self.update_camera(queue);
    }

    pub fn set_parallel_scale(&mut self, half_w: f32, half_h: f32, queue: &wgpu::Queue) {
        self.camera.ortho_half_w = half_w;
        self.camera.ortho_half_h = half_h;
        self.update_camera(queue);
    }

    /// Render the scatter to an offscreen texture and return (width, height, rgba_bytes).
    ///
    /// BGRA surface formats are swapped to RGBA before returning.
    pub fn screenshot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(u32, u32, Vec<u8>), String> {
        let (w, h) = (self.width, self.height);
        if w == 0 || h == 0 {
            return Err("scatter widget has zero dimensions".to_string());
        }

        let bytes_per_px = 4u32;
        let unpadded_row = w * bytes_per_px;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row = (unpadded_row + align - 1) & !(align - 1);

        // Reuse cached GPU resources when dimensions match.
        if self
            .screenshot_cache
            .as_ref()
            .map_or(true, |c| c.w != w || c.h != h)
        {
            let color_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("scatter-screenshot-color"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let color_view = color_tex.create_view(&wgpu::TextureViewDescriptor::default());
            let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("scatter-screenshot-depth"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::DEPTH_STENCIL_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let depth_view = depth_tex.create_view(&wgpu::TextureViewDescriptor::default());
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-screenshot-readback"),
                size: (padded_row * h) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.screenshot_cache = Some(ScreenshotCache {
                w,
                h,
                color_tex,
                color_view,
                _depth_tex: depth_tex,
                depth_view,
                readback,
                padded_row,
            });
        }

        let cache = self.screenshot_cache.take().unwrap();

        // Update uniforms exactly as in update_camera, but against the offscreen dims.
        let uniforms = self.uniforms_for_size(w, h);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Override window-global coords so render() uses offscreen dims.
        // Must happen before the render pass block so we can restore AFTER pass is dropped.
        let saved_offset = self.offset;
        let saved_scissor_offset = self.scissor_offset;
        let saved_scissor_size = self.scissor_size;
        self.offset = [0.0, 0.0];
        self.scissor_offset = [0, 0];
        self.scissor_size = [w, h];

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("scatter-screenshot"),
        });
        self.prepare_render_pipelines(device);
        {
            let bg = self
                .chrome
                .background_color
                .unwrap_or([0.12, 0.12, 0.12, 1.0]);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scatter-screenshot-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &cache.color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0] as f64,
                            g: bg[1] as f64,
                            b: bg[2] as f64,
                            a: bg[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &cache.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Store,
                    }),
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            self.render(&mut pass);
        } // pass dropped here — releases the &self borrow from render()

        // Restore coords now that pass (and its lifetime constraint on self) is gone.
        self.offset = saved_offset;
        self.scissor_offset = saved_scissor_offset;
        self.scissor_size = saved_scissor_size;
        self.last_uniforms = None;
        self.update_camera(queue);

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &cache.color_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &cache.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(cache.padded_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        cache
            .readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                tx.send(r).ok();
            });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv()
            .unwrap()
            .map_err(|e| format!("screenshot map_async: {e:?}"))?;

        let raw = cache.readback.slice(..).get_mapped_range();
        let is_bgra = matches!(
            self.surface_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let pixels = if !is_bgra && cache.padded_row == w * bytes_per_px {
            raw.to_vec()
        } else {
            let mut out = Vec::with_capacity(w as usize * h as usize * 4);
            for row in 0..h as usize {
                let start = row * cache.padded_row as usize;
                let row_bytes = &raw[start..start + w as usize * 4];
                if is_bgra {
                    for px in row_bytes.chunks_exact(4) {
                        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                    }
                } else {
                    out.extend_from_slice(row_bytes);
                }
            }
            out
        };
        drop(raw);
        cache.readback.unmap();
        self.screenshot_cache = Some(cache);
        Ok((w, h, pixels))
    }

    pub fn set_camera_state(
        &mut self,
        state: crate::scatter::camera::CameraState,
        queue: &wgpu::Queue,
    ) {
        self.camera.apply_state(state);
        self.update_camera(queue);
    }

    pub fn camera_state(&self) -> crate::scatter::camera::CameraState {
        self.camera.state()
    }

    /// Fit the camera to a data bounding box and save as the reset target.
    pub fn fit_to_bounds(&mut self, min: glam::Vec3, max: glam::Vec3, queue: &wgpu::Queue) {
        let center = (min + max) * 0.5;
        let half_diag = (max - min).length() * 0.5;
        let radius = half_diag.max(0.1);
        self.fit_center = center;
        self.fit_radius = radius;
        let aspect = self.width as f32 / self.height.max(1) as f32;
        self.camera = self.camera.fit_preserving_view(center, radius, aspect);
        self.update_camera(queue);
    }

    /// Update chrome state and mark grid dirty so the next `refresh_grid` rebuilds.
    pub fn set_chrome(&mut self, chrome: ScatterChromeState) {
        self.chrome = chrome;
        self.chrome_dirty = true;
    }

    fn grid_label_projection_key(&self) -> Option<GridLabelProjectionCacheKey> {
        if self.width == 0
            || self.height == 0
            || self.scissor_size[0] == 0
            || self.scissor_size[1] == 0
        {
            return None;
        }
        Some(GridLabelProjectionCacheKey {
            view_proj: self.camera.view_proj(),
            width: self.width,
            height: self.height,
            offset: self.offset,
            scissor_offset: self.scissor_offset,
            scissor_size: self.scissor_size,
        })
    }

    /// Rebuild grid geometry if bounds, camera face, or chrome changed.
    /// Call after any operation that could change the visible grid.
    pub fn refresh_grid(
        &mut self,
        data_min: glam::Vec3,
        data_max: glam::Vec3,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if !self.chrome.grid_visible {
            if self.grid_vertex_count != 0 || !self.grid_labels.is_empty() {
                self.mark_scene_dirty();
            }
            self.grid_vertex_count = 0;
            self.grid_labels.clear();
            self.reproject_grid_labels();
            return;
        }

        let eye = self.camera.position();
        let (nice_min, nice_max) = if self.chrome.grid_sticky {
            sticky_nice_bounds(self.grid_display_bounds, data_min, data_max)
        } else {
            grid::nice_bounds(data_min, data_max)
        };
        self.grid_display_bounds = Some((nice_min, nice_max));
        let display_center = (nice_min + nice_max) * 0.5;
        let display_extent = nice_max - nice_min;
        let bits = if self.chrome.grid_sticky {
            stable_face_bits(eye, display_center, display_extent, self.last_face_bits)
        } else {
            grid::face_bits(eye, display_center)
        };
        let bounds_key = Some((nice_min, nice_max));
        let ortho_scale = if self.camera.ortho_half_w > 0.0 && self.camera.ortho_half_h > 0.0 {
            Some((self.camera.ortho_half_w, self.camera.ortho_half_h))
        } else {
            None
        };

        if !self.chrome_dirty
            && bits == self.last_face_bits
            && self.last_grid_bounds == bounds_key
            && self.last_grid_ortho_scale == ortho_scale
        {
            if self.last_grid_label_projection_key != self.grid_label_projection_key() {
                self.reproject_grid_labels();
            }
            return; // nothing changed
        }

        self.chrome_dirty = false;
        self.last_face_bits = bits;
        self.last_grid_bounds = bounds_key;
        self.last_grid_ortho_scale = ortho_scale;

        let geo = build_grid(
            data_min,
            data_max,
            nice_min,
            nice_max,
            self.chrome.tick_override,
            self.chrome.axis_visible,
            eye,
            &self.chrome.axis_labels,
            self.chrome.major_planes,
            self.chrome.minor_planes,
            self.chrome.grid_all_edges,
            ortho_scale,
        );
        let GridGeometry { vertices, labels } = geo;

        // Upload line vertices
        let line_bytes: &[u8] = bytemuck::cast_slice(&vertices);
        let line_size = line_bytes.len() as u64;
        if line_size == 0 {
            self.grid_vertex_count = 0;
        } else {
            if self.line_vertex_buffer.is_none() || line_size > self.line_vertex_cap {
                let cap = (line_size * 2).max(256 * 1024);
                self.line_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("scatter-lines-vb"),
                    size: cap,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.line_vertex_cap = cap;
            }
            queue.write_buffer(self.line_vertex_buffer.as_ref().unwrap(), 0, line_bytes);
            self.grid_vertex_count = vertices.len() as u32;
            self.mark_scene_dirty();
        }

        self.grid_labels = labels;
        self.reproject_grid_labels();
    }

    fn reproject_grid_labels(&mut self) {
        let mut overlay_labels = std::mem::take(&mut self.reproject_overlay_scratch);
        overlay_labels.clear();
        if self.pending_overlay_offset < self.pending_labels.len() {
            overlay_labels.extend(self.pending_labels.drain(self.pending_overlay_offset..));
        }
        self.pending_labels.clear();
        self.pending_overlay_offset = 0;
        if self.width == 0 || self.height == 0 {
            self.pending_labels.extend(overlay_labels.drain(..));
            self.reproject_overlay_scratch = overlay_labels;
            self.last_grid_label_projection_key = None;
            return;
        }
        let vp = self.camera.view_proj();
        let w = self.width as f32;
        let h = self.height as f32;
        // Scissor clip bounds in window space
        let sx0 = self.scissor_offset[0] as f32;
        let sy0 = self.scissor_offset[1] as f32;
        let sx1 = sx0 + self.scissor_size[0] as f32;
        let sy1 = sy0 + self.scissor_size[1] as f32;
        let offset = self.offset;
        const MIN_TICK_LABEL_PUSH_PX: f32 = 16.0;

        let visible_in_scissor = move |local_x: f32, local_y: f32| -> bool {
            let screen_x = offset[0] + local_x;
            let screen_y = offset[1] + local_y;
            screen_x >= sx0 && screen_x <= sx1 && screen_y >= sy0 && screen_y <= sy1
        };

        let mut tick_label_rects = std::mem::take(&mut self.grid_label_rects_scratch);
        tick_label_rects.clear();
        let mut title_projections = std::mem::take(&mut self.grid_title_projection_scratch);
        title_projections.clear();

        for anchor in &self.grid_labels {
            let p = anchor.world_pos;
            let clip = vp * glam::Vec4::new(p.x, p.y, p.z, 1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            // Allow a small margin outside [-1,1] so labels near the edge are kept
            if ndc.x.abs() > 1.3 || ndc.y.abs() > 1.3 || ndc.z < 0.0 || ndc.z > 1.0 {
                continue;
            }
            let mut local_x = (ndc.x * 0.5 + 0.5) * w;
            let mut local_y = (0.5 - ndc.y * 0.5) * h;
            if anchor.is_axis_title {
                let mut push_dir = glam::Vec2::ZERO;
                let tp = anchor.tick_pos;
                let tick_clip = vp * glam::Vec4::new(tp.x, tp.y, tp.z, 1.0);
                if tick_clip.w > 0.0 {
                    let tick_ndc = tick_clip.truncate() / tick_clip.w;
                    let tick_x = (tick_ndc.x * 0.5 + 0.5) * w;
                    let tick_y = (0.5 - tick_ndc.y * 0.5) * h;
                    push_dir = glam::Vec2::new(local_x - tick_x, local_y - tick_y);
                }
                if self.chrome.axis_visible[2] {
                    let center_x = w * 0.5;
                    let center_y = h * 0.5;
                    let dx = local_x - center_x;
                    let dy = local_y - center_y;
                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                    local_x += dx / len * 24.0;
                    local_y += dy / len * 24.0;
                    if push_dir.length_squared() < 1.0 {
                        push_dir = glam::Vec2::new(local_x - center_x, local_y - center_y);
                    }
                }
                title_projections.push(GridLabelProjection {
                    local_x,
                    local_y,
                    text: anchor.text.clone(),
                    is_title: true,
                    push_dir,
                });
            } else {
                let tp = anchor.tick_pos;
                let tick_clip = vp * glam::Vec4::new(tp.x, tp.y, tp.z, 1.0);
                if tick_clip.w > 0.0 {
                    let tick_ndc = tick_clip.truncate() / tick_clip.w;
                    let tick_x = (tick_ndc.x * 0.5 + 0.5) * w;
                    let tick_y = (0.5 - tick_ndc.y * 0.5) * h;
                    let dx = local_x - tick_x;
                    let dy = local_y - tick_y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 1.0 {
                        continue;
                    }
                    if len < MIN_TICK_LABEL_PUSH_PX {
                        local_x = tick_x + dx / len * MIN_TICK_LABEL_PUSH_PX;
                        local_y = tick_y + dy / len * MIN_TICK_LABEL_PUSH_PX;
                    }
                }
                if !visible_in_scissor(local_x, local_y) {
                    continue;
                }
                tick_label_rects.push(scatter_label_rect(local_x, local_y, &anchor.text, false));
                self.pending_labels.push(ProjectedLabel {
                    screen_x: self.offset[0] + local_x,
                    screen_y: self.offset[1] + local_y,
                    text: anchor.text.clone(),
                    is_title: false,
                    color: None,
                    font_size: None,
                    anchor: "scatter-axis-center".into(),
                });
            }
        }

        let mut occupied_rects = tick_label_rects;
        for title in title_projections.drain(..) {
            let (local_x, local_y) = push_scatter_title_away_from_rects(
                title.local_x,
                title.local_y,
                &title.text,
                title.push_dir,
                &occupied_rects,
            );
            if !visible_in_scissor(local_x, local_y) {
                continue;
            }
            occupied_rects.push(scatter_label_rect(local_x, local_y, &title.text, true));
            self.pending_labels.push(ProjectedLabel {
                screen_x: self.offset[0] + local_x,
                screen_y: self.offset[1] + local_y,
                text: title.text,
                is_title: title.is_title,
                color: None,
                font_size: None,
                anchor: "scatter-axis-center".into(),
            });
        }
        // Mark where grid-axis labels end; refresh_overlays() will truncate here before appending.
        self.pending_overlay_offset = self.pending_labels.len();
        self.pending_labels.extend(overlay_labels.drain(..));
        occupied_rects.clear();
        title_projections.clear();
        self.grid_label_rects_scratch = occupied_rects;
        self.grid_title_projection_scratch = title_projections;
        self.reproject_overlay_scratch = overlay_labels;
        self.last_grid_label_projection_key = self.grid_label_projection_key();
    }

    /// Rebuild screen-space overlay geometry (orientation axes, legend swatches,
    /// scalar bar strip) from the current chrome state and camera orientation.
    ///
    /// Must be called whenever the camera changes or chrome state changes.
    pub fn refresh_overlays(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // Discard previously-appended overlay labels; preserve the grid-axis prefix.
        self.pending_labels.truncate(self.pending_overlay_offset);

        // Background fill: 6 NDC-space vertices (2 triangles) covering the viewport.
        // Drawn with bg_pipeline (TriangleList) so viewport/scissor clips it — not a pass clear.
        if let Some(bg) = self.chrome.background_color {
            let [r, g, b, _] = bg;
            let col = [r, g, b];
            let bg_verts = [
                OverlayVertex {
                    position: [-1.0, 1.0],
                    color: col,
                },
                OverlayVertex {
                    position: [1.0, 1.0],
                    color: col,
                },
                OverlayVertex {
                    position: [-1.0, -1.0],
                    color: col,
                },
                OverlayVertex {
                    position: [-1.0, -1.0],
                    color: col,
                },
                OverlayVertex {
                    position: [1.0, 1.0],
                    color: col,
                },
                OverlayVertex {
                    position: [1.0, -1.0],
                    color: col,
                },
            ];
            let bytes: &[u8] = bytemuck::cast_slice(&bg_verts);
            let size = bytes.len() as u64;
            if self.bg_vertex_buffer.is_none() || size > self.bg_vertex_cap {
                let cap = (size * 2).max(4 * 1024);
                self.bg_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("scatter-bg-vb"),
                    size: cap,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.bg_vertex_cap = cap;
            }
            queue.write_buffer(self.bg_vertex_buffer.as_ref().unwrap(), 0, bytes);
            self.bg_vertex_count = 6;
            self.mark_scene_dirty();
        } else {
            if self.bg_vertex_count != 0 {
                self.mark_scene_dirty();
            }
            self.bg_vertex_count = 0;
        }

        let has_orient = self.chrome.orientation_axes_visible;
        let has_legend = self.chrome.legend.visible && !self.chrome.legend.entries.is_empty();
        let has_scalar = self.chrome.scalar_bar.visible;

        if !has_orient
            && !has_legend
            && !has_scalar
            && self.selection_rect.is_none()
            && self.selection_polygon.is_none()
            && self.hover_label.is_none()
            && self.user_labels.is_empty()
        {
            if self.overlay_vertex_count != 0 {
                self.mark_scene_dirty();
            }
            self.overlay_vertex_count = 0;
            return;
        }

        let w = self.width as f32;
        let h = self.height as f32;
        if w == 0.0 || h == 0.0 {
            if self.overlay_vertex_count != 0 {
                self.mark_scene_dirty();
            }
            self.overlay_vertex_count = 0;
            return;
        }

        let mut verts = std::mem::take(&mut self.overlay_vertices_scratch);
        verts.clear();

        // Helper: convert viewport-local px coords → NDC
        let to_ndc = |px: f32, py: f32| -> [f32; 2] { overlay_to_ndc(w, h, px, py) };

        // ── Orientation axes ─────────────────────────────────────────────────
        if has_orient {
            // Project world-basis vectors through the view rotation only
            // (ignore translation and projection; just rotate).
            let view = self.camera.view_matrix();
            let rot = glam::Mat3::from_mat4(view);
            // Widget: 52×52 px in bottom-left corner, 10px margin
            let ox: f32 = 30.0;
            let oy: f32 = h - 30.0;
            let arm: f32 = 22.0;
            let axes: [([f32; 3], [f32; 3]); 3] = [
                ([1.0, 0.0, 0.0], [0.85, 0.25, 0.25]),
                ([0.0, 1.0, 0.0], [0.25, 0.78, 0.35]),
                ([0.0, 0.0, 1.0], [0.30, 0.55, 0.95]),
            ];
            for (world_dir, color) in &axes {
                let d = rot * glam::Vec3::from(*world_dir);
                // d.x is right, d.y is up in view space → screen coords
                let ex = ox + d.x * arm;
                let ey = oy - d.y * arm;
                verts.push(OverlayVertex {
                    position: to_ndc(ox, oy),
                    color: *color,
                });
                verts.push(OverlayVertex {
                    position: to_ndc(ex, ey),
                    color: *color,
                });
            }
        }

        // ── Legend swatches ──────────────────────────────────────────────────
        if has_legend {
            let entries = &self.chrome.legend.entries;
            let entry_h: f32 = 16.0;
            let swatch_w: f32 = 12.0;
            let margin: f32 = 10.0;
            let title_h = if self.chrome.legend.title.is_some() {
                entry_h
            } else {
                0.0
            };
            let total_h = entries.len() as f32 * entry_h + title_h;
            // Anchor position
            let (legend_x, legend_y) = match self.chrome.legend.position {
                LegendPosition::TopRight => {
                    let y = if has_scalar {
                        let (_, _, bar_top, bar_h) = scalar_bar_geometry(w, h);
                        bar_top + bar_h + 10.0
                    } else {
                        margin
                    };
                    (w - margin - swatch_w - 80.0, y)
                }
                LegendPosition::TopLeft => (margin, margin),
                LegendPosition::BottomRight => (w - margin - swatch_w - 80.0, h - margin - total_h),
                LegendPosition::BottomLeft => {
                    let orientation_reserved = if has_orient { 62.0 } else { 0.0 };
                    (
                        margin,
                        (h - margin - total_h - orientation_reserved).max(margin),
                    )
                }
            };
            if let Some(title) = &self.chrome.legend.title {
                self.pending_labels.push(ProjectedLabel {
                    screen_x: self.offset[0] + legend_x,
                    screen_y: self.offset[1] + legend_y,
                    text: title.clone(),
                    is_title: true,
                    color: None,
                    font_size: None,
                    anchor: "left".into(),
                });
            }
            let entries_y = legend_y + title_h;
            for (i, entry) in entries.iter().enumerate() {
                let sy = entries_y + i as f32 * entry_h + entry_h * 0.25;
                let ex = legend_x + swatch_w;
                let ey = sy + entry_h * 0.5;
                let c = entry.color;
                // Draw a small colored square outline (4 sides = 8 vertices for LineList)
                let corners = [
                    to_ndc(legend_x, sy),
                    to_ndc(ex, sy),
                    to_ndc(ex, ey),
                    to_ndc(legend_x, ey),
                ];
                for j in 0..4usize {
                    verts.push(OverlayVertex {
                        position: corners[j],
                        color: c,
                    });
                    verts.push(OverlayVertex {
                        position: corners[(j + 1) % 4],
                        color: c,
                    });
                }
                // Text label goes through pending_labels
                let tx = self.offset[0] + ex + 5.0;
                let ty = self.offset[1] + sy + (entry_h * 0.5 - 5.0);
                self.pending_labels.push(ProjectedLabel {
                    screen_x: tx,
                    screen_y: ty,
                    text: entry.label.clone(),
                    is_title: false,
                    color: None,
                    font_size: None,
                    anchor: "left".into(),
                });
            }
        }

        // ── Scalar bar ───────────────────────────────────────────────────────
        if has_scalar {
            let (_bar_x1, bar_x2, bar_top, bar_h) = scalar_bar_geometry(w, h);
            let scalar_cache_matches =
                self.scalar_bar_vertex_cache_key
                    .as_ref()
                    .is_some_and(|key| {
                        key.width == self.width
                            && key.height == self.height
                            && key.colormap == self.chrome.scalar_bar.colormap
                    });
            if !scalar_cache_matches {
                let colormap = self.chrome.scalar_bar.colormap.clone();
                let mut cached = std::mem::take(&mut self.scalar_bar_vertex_cache);
                cached.clear();
                push_scalar_bar_vertices(w, h, &colormap, &mut cached);
                self.scalar_bar_vertex_cache = cached;
                self.scalar_bar_vertex_cache_key = Some(ScalarBarVertexCacheKey {
                    width: self.width,
                    height: self.height,
                    colormap,
                });
            }
            let sb = &self.chrome.scalar_bar;
            verts.extend_from_slice(&self.scalar_bar_vertex_cache);
            // Tick labels: up to 6 ticks (top + intermediates + bottom).
            // Minimum pixel gap between labels; skip intermediates when bar is too short.
            let min_label_gap_px: f32 = 18.0;
            let max_ticks: usize = if bar_h > 0.0 {
                ((bar_h / min_label_gap_px) as usize).max(2).min(6)
            } else {
                2
            };
            let tick_vals = scalar_bar_tick_values(sb.vmin, sb.vmax, sb.log_scale, max_ticks);
            let label_x = self.offset[0] + bar_x2 + SCALAR_BAR_TICK_GAP_PX;
            for (i, &val) in tick_vals.iter().enumerate() {
                let t = if tick_vals.len() == 1 {
                    0.5 // single tick (constant-value bar): center on the bar
                } else {
                    i as f32 / (tick_vals.len() - 1) as f32
                };
                // t=0 → vmin at bottom; t=1 → vmax at top (bar is drawn top-to-bottom).
                let label_y = self.offset[1] + bar_top + (1.0 - t) * bar_h - 5.0;
                self.pending_labels.push(ProjectedLabel {
                    screen_x: label_x,
                    screen_y: label_y,
                    text: format_scalar_bar_tick(val, sb.log_scale),
                    is_title: false,
                    color: None,
                    font_size: None,
                    anchor: "top-left".into(),
                });
            }
            if let Some(title) = &sb.title {
                self.pending_labels.push(ProjectedLabel {
                    // The text renderer bounds this region to the scatter viewport,
                    // right-aligns it over the bar/tick area, and ellipsizes overflow.
                    screen_x: self.offset[0] + w - SCALAR_BAR_TITLE_RIGHT_INSET_PX,
                    screen_y: self.offset[1] + SCALAR_BAR_TITLE_TOP_PX,
                    text: title.clone(),
                    is_title: true,
                    color: None,
                    font_size: None,
                    anchor: "scalar-bar-title".into(),
                });
            }
        }

        // ── Selection rectangle ────────────────────────────────────────────────
        if let Some(rect) = self.selection_rect {
            let [x0, y0, x1, y1] = rect;
            let col = [0.35_f32, 0.75, 1.0];
            for (ax, ay, bx, by) in [
                (x0, y0, x1, y0),
                (x1, y0, x1, y1),
                (x1, y1, x0, y1),
                (x0, y1, x0, y0),
            ] {
                verts.push(OverlayVertex {
                    position: to_ndc(ax, ay),
                    color: col,
                });
                verts.push(OverlayVertex {
                    position: to_ndc(bx, by),
                    color: col,
                });
            }
        }

        // ── Lasso polygon ─────────────────────────────────────────────────────
        if let Some(poly) = &self.selection_polygon {
            if poly.len() >= 2 {
                let col = [0.35_f32, 0.75, 1.0];
                for i in 0..poly.len() {
                    let [ax, ay] = poly[i];
                    let [bx, by] = poly[(i + 1) % poly.len()];
                    verts.push(OverlayVertex {
                        position: to_ndc(ax, ay),
                        color: col,
                    });
                    verts.push(OverlayVertex {
                        position: to_ndc(bx, by),
                        color: col,
                    });
                }
            }
        }

        self.upload_overlay_vertices(&verts, device, queue);
        verts.clear();
        self.overlay_vertices_scratch = verts;

        // Project world-space user labels to screen space and append to pending_labels.
        if !self.user_labels.is_empty() && self.width > 0 && self.height > 0 {
            let vp = self.camera.view_proj();
            let w = self.width as f32;
            let h = self.height as f32;
            let sx0 = self.scissor_offset[0] as f32;
            let sy0 = self.scissor_offset[1] as f32;
            let sx1 = sx0 + self.scissor_size[0] as f32;
            let sy1 = sy0 + self.scissor_size[1] as f32;
            for label in &self.user_labels {
                if !label.visible {
                    continue;
                }
                let p = label.position;
                let clip = vp * glam::Vec4::new(p.x, p.y, p.z, 1.0);
                if clip.w <= 0.0 {
                    continue;
                }
                let ndc = clip.truncate() / clip.w;
                if ndc.x.abs() > 1.3 || ndc.y.abs() > 1.3 || ndc.z < 0.0 || ndc.z > 1.0 {
                    continue;
                }
                let local_x = (ndc.x * 0.5 + 0.5) * w;
                let local_y = (0.5 - ndc.y * 0.5) * h;
                let screen_x = self.offset[0] + local_x;
                let screen_y = self.offset[1] + local_y;
                if screen_x < sx0 || screen_x > sx1 || screen_y < sy0 || screen_y > sy1 {
                    continue;
                }
                self.pending_labels.push(ProjectedLabel {
                    screen_x,
                    screen_y,
                    text: label.text.clone(),
                    is_title: false,
                    color: Some(label.color),
                    font_size: Some(label.size),
                    anchor: label.anchor.clone().into(),
                });
            }
        }

        // Hover tooltip label appended last so it renders on top of everything else.
        if let Some(ref hl) = self.hover_label {
            self.pending_labels.push(ProjectedLabel {
                screen_x: hl.screen_x,
                screen_y: hl.screen_y,
                text: hl.text.clone(),
                is_title: false,
                color: None,
                font_size: None,
                anchor: hl.anchor.clone(),
            });
        }
    }

    fn upload_overlay_vertices(
        &mut self,
        verts: &[OverlayVertex],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let bytes: &[u8] = bytemuck::cast_slice(verts);
        let size = bytes.len() as u64;
        if size == 0 {
            if self.overlay_vertex_count != 0 {
                self.mark_scene_dirty();
            }
            self.overlay_vertex_count = 0;
            return;
        }
        if self.overlay_vertex_buffer.is_none() || size > self.overlay_vertex_cap {
            let cap = (size * 2).max(64 * 1024);
            self.overlay_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-overlay-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.overlay_vertex_cap = cap;
        }
        queue.write_buffer(self.overlay_vertex_buffer.as_ref().unwrap(), 0, bytes);
        self.overlay_vertex_count = verts.len() as u32;
        self.mark_scene_dirty();
    }

    // ── User labels ──────────────────────────────────────────────────────────

    pub fn add_user_label(
        &mut self,
        id: u32,
        position: glam::Vec3,
        text: String,
        color: [f32; 3],
        size: f32,
        anchor: String,
    ) {
        self.user_labels.retain(|l| l.id != id);
        self.user_labels.push(UserLabel {
            id,
            position,
            text,
            color,
            size,
            anchor,
            visible: true,
        });
    }

    pub fn update_user_label(
        &mut self,
        id: u32,
        position: Option<glam::Vec3>,
        text: Option<String>,
        color: Option<[f32; 3]>,
        size: Option<f32>,
        anchor: Option<String>,
    ) {
        if let Some(label) = self.user_labels.iter_mut().find(|l| l.id == id) {
            if let Some(p) = position {
                label.position = p;
            }
            if let Some(t) = text {
                label.text = t;
            }
            if let Some(c) = color {
                label.color = c;
            }
            if let Some(s) = size {
                label.size = s;
            }
            if let Some(a) = anchor {
                label.anchor = a;
            }
        }
    }

    pub fn remove_user_label(&mut self, id: u32) {
        self.user_labels.retain(|l| l.id != id);
    }

    pub fn set_user_label_visible(&mut self, id: u32, visible: bool) {
        if let Some(label) = self.user_labels.iter_mut().find(|l| l.id == id) {
            label.visible = visible;
        }
    }

    pub fn clear_user_labels(&mut self) {
        self.user_labels.clear();
    }

    // ── Line overlays ────────────────────────────────────────────────────────

    fn line_vertices_from_segments(segments: Vec<[f32; 6]>, color: [f32; 3]) -> Vec<LineVertex> {
        let mut vertices = Vec::with_capacity(segments.len() * 2);
        for [x0, y0, z0, x1, y1, z1] in segments {
            vertices.push(LineVertex {
                position: [x0, y0, z0],
                color,
            });
            vertices.push(LineVertex {
                position: [x1, y1, z1],
                color,
            });
        }
        vertices
    }

    pub fn add_line_overlay(&mut self, id: u32, segments: Vec<[f32; 6]>, color: [f32; 3]) {
        self.line_overlays.retain(|o| o.id != id);
        let vertices = Self::line_vertices_from_segments(segments, color);
        self.line_overlays.push(LineOverlay {
            id,
            vertices,
            visible: true,
        });
    }

    pub fn update_line_overlay(&mut self, id: u32, segments: Vec<[f32; 6]>, color: [f32; 3]) {
        let vertices = Self::line_vertices_from_segments(segments, color);
        if let Some(overlay) = self.line_overlays.iter_mut().find(|o| o.id == id) {
            overlay.vertices = vertices;
        }
    }

    pub fn add_box_overlay(&mut self, id: u32, bounds: [f32; 6], color: [f32; 3]) {
        self.line_overlays.retain(|o| o.id != id);
        let [xmin, xmax, ymin, ymax, zmin, zmax] = bounds;
        let corners: [[f32; 3]; 8] = [
            [xmin, ymin, zmin],
            [xmax, ymin, zmin],
            [xmax, ymax, zmin],
            [xmin, ymax, zmin],
            [xmin, ymin, zmax],
            [xmax, ymin, zmax],
            [xmax, ymax, zmax],
            [xmin, ymax, zmax],
        ];
        let edges: [(usize, usize); 12] = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        let mut vertices = Vec::with_capacity(24);
        for (a, b) in edges {
            vertices.push(LineVertex {
                position: corners[a],
                color,
            });
            vertices.push(LineVertex {
                position: corners[b],
                color,
            });
        }
        self.line_overlays.push(LineOverlay {
            id,
            vertices,
            visible: true,
        });
    }

    pub fn remove_line_overlay(&mut self, id: u32) {
        self.line_overlays.retain(|o| o.id != id);
    }

    pub fn set_line_overlay_visible(&mut self, id: u32, visible: bool) {
        if let Some(overlay) = self.line_overlays.iter_mut().find(|o| o.id == id) {
            overlay.visible = visible;
        }
    }

    pub fn clear_line_overlays(&mut self) {
        self.line_overlays.clear();
    }

    // ── Multi-actor API ──────────────────────────────────────────────────────

    /// Add or replace an extra point actor by ``id``.
    pub fn add_actor(
        &mut self,
        id: u32,
        pts: Vec<PointInstance>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        let (mn, mx) = PointActor::compute_bounds(&pts);
        let actor = self
            .extra_actors
            .entry(id)
            .or_insert_with(|| PointActor::new(id));
        let build_lod =
            self.lod_active && self.lod_enabled && (pts.len() as u32) > self.lod_threshold;
        actor.upload(&pts, build_lod, self.lod_factor, device, queue);
        actor.points = pts;
        actor.pick_cache = None; // belt-and-suspenders: upload() already clears, but points just changed too
        actor.data_min = mn;
        actor.data_max = mx;
        actor.visible = true;
        self.mark_scene_dirty();
        self.recompute_point_size_scale();
        self.update_camera(queue);
    }

    /// Update (replace) an existing extra actor's points.
    pub fn update_actor(
        &mut self,
        id: u32,
        pts: Vec<PointInstance>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if let Some(actor) = self.extra_actors.get_mut(&id) {
            let (mn, mx) = PointActor::compute_bounds(&pts);
            let build_lod =
                self.lod_active && self.lod_enabled && (pts.len() as u32) > self.lod_threshold;
            actor.upload(&pts, build_lod, self.lod_factor, device, queue);
            actor.points = pts;
            actor.data_min = mn;
            actor.data_max = mx;
            actor.hover_meta = Vec::new();
            self.mark_scene_dirty();
            self.recompute_point_size_scale();
            self.update_camera(queue);
        }
    }

    pub fn remove_actor(&mut self, id: u32) {
        if self.extra_actors.remove(&id).is_some() {
            self.mark_scene_dirty();
        }
    }

    pub fn set_actor_visible(&mut self, id: u32, visible: bool) {
        if let Some(actor) = self.extra_actors.get_mut(&id) {
            if actor.visible != visible {
                actor.visible = visible;
                self.mark_scene_dirty();
            }
        }
    }

    pub fn clear_extra_actors(&mut self) {
        if !self.extra_actors.is_empty() {
            self.extra_actors.clear();
            self.mark_scene_dirty();
        }
    }

    /// Returns the union bounds of all visible extra actors, or None if there are none.
    pub fn merged_extra_bounds(&self) -> Option<(glam::Vec3, glam::Vec3)> {
        let mut mn = glam::Vec3::splat(f32::MAX);
        let mut mx = glam::Vec3::splat(f32::MIN);
        let mut any = false;
        for actor in self.extra_actors.values() {
            if actor.visible && actor.point_count > 0 {
                mn = mn.min(actor.data_min);
                mx = mx.max(actor.data_max);
                any = true;
            }
        }
        if any {
            Some((mn, mx))
        } else {
            None
        }
    }

    /// Pre-allocate a fixed-capacity stream actor.
    pub fn add_stream_actor(
        &mut self,
        id: u32,
        max_points: u32,
        mode: StreamMode,
        device: &wgpu::Device,
    ) {
        let size = (max_points as u64) * (std::mem::size_of::<PointInstance>() as u64);
        let vertex_buffer = if size > 0 {
            Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-stream-vb"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }))
        } else {
            None
        };
        let mut actor = PointActor::new(id);
        actor.vertex_buffer = vertex_buffer;
        actor.vertex_cap = size;
        actor.stream_mode = Some(mode);
        actor.points_opaque = true;
        actor.stream_capacity = max_points;
        actor.stream_write_offset = 0;
        actor.points = vec![
            PointInstance {
                position: [0.0; 3],
                size: 0.0,
                color: [0.0; 3],
                alpha: 0.0
            };
            max_points as usize
        ];
        self.extra_actors.insert(id, actor);
        self.mark_scene_dirty();
    }

    /// Append or ring-overwrite points into a stream actor.
    pub fn stream_actor(&mut self, id: u32, pts: &[PointInstance], queue: &wgpu::Queue) {
        let Some(actor) = self.extra_actors.get_mut(&id) else {
            return;
        };
        let Some(mode) = actor.stream_mode else {
            return;
        };
        let Some(ref buf) = actor.vertex_buffer else {
            return;
        };
        let cap = actor.stream_capacity as usize;
        if cap == 0 || pts.is_empty() {
            return;
        }
        let stride = std::mem::size_of::<PointInstance>();
        match mode {
            StreamMode::Append => {
                let remaining = cap.saturating_sub(actor.point_count as usize);
                let to_write = pts.len().min(remaining);
                if to_write == 0 {
                    return;
                }
                let byte_offset = (actor.point_count as u64) * (stride as u64);
                queue.write_buffer(buf, byte_offset, bytemuck::cast_slice(&pts[..to_write]));
                for (i, p) in pts[..to_write].iter().enumerate() {
                    actor.points[actor.point_count as usize + i] = *p;
                }
                actor.point_count = (actor.point_count + to_write as u32).min(cap as u32);
            }
            StreamMode::Ring => {
                let mut offset = actor.stream_write_offset as usize;
                for &pt in pts {
                    let byte_off = (offset as u64) * (stride as u64);
                    queue.write_buffer(buf, byte_off, bytemuck::cast_slice(&[pt]));
                    actor.points[offset] = pt;
                    offset = (offset + 1) % cap;
                }
                actor.stream_write_offset = offset as u32;
                actor.point_count = cap.min(actor.point_count as usize + pts.len()) as u32;
            }
        }
        // Update bounds conservatively from CPU cache
        let (mn, mx) = PointActor::compute_bounds(&actor.points[..actor.point_count as usize]);
        actor.data_min = mn;
        actor.data_max = mx;
        actor.points_opaque = points_are_opaque(&actor.points[..actor.point_count as usize]);
        self.mark_scene_dirty();
        self.recompute_point_size_scale();
        self.update_camera(queue);
    }

    pub fn clear_stream_actor(&mut self, id: u32, queue: &wgpu::Queue) {
        if let Some(actor) = self.extra_actors.get_mut(&id) {
            actor.point_count = 0;
            actor.points_opaque = true;
            actor.stream_write_offset = 0;
            actor.data_min = glam::Vec3::splat(f32::MAX);
            actor.data_max = glam::Vec3::splat(f32::MIN);
            self.mark_scene_dirty();
            self.recompute_point_size_scale();
            self.update_camera(queue);
        }
    }

    /// Rebuild the user line vertex buffer from the current `line_overlays`.
    pub fn refresh_user_lines(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let all_verts: Vec<LineVertex> = self
            .line_overlays
            .iter()
            .filter(|o| o.visible)
            .flat_map(|o| o.vertices.iter().copied())
            .collect();

        let bytes: &[u8] = bytemuck::cast_slice(&all_verts);
        let size = bytes.len() as u64;
        if size == 0 {
            if self.user_line_vertex_count != 0 {
                self.mark_scene_dirty();
            }
            self.user_line_vertex_count = 0;
            return;
        }
        if self.user_line_vertex_buffer.is_none() || size > self.user_line_vertex_cap {
            let cap = (size * 2).max(64 * 1024);
            self.user_line_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("scatter-user-lines-vb"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.user_line_vertex_cap = cap;
        }
        queue.write_buffer(self.user_line_vertex_buffer.as_ref().unwrap(), 0, bytes);
        self.user_line_vertex_count = all_verts.len() as u32;
        self.mark_scene_dirty();
    }

    // ── Mesh overlay management (Phase 6) ────────────────────────────────────

    fn ensure_mesh_pipelines(&mut self, device: &wgpu::Device) {
        if self.mesh_solid_pipeline.is_some()
            && self.mesh_transparent_pipeline.is_some()
            && self.mesh_wire_pipeline.is_some()
        {
            return;
        }

        let mesh_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scatter-mesh"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
        });
        let mesh_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &MESH_VERTEX_ATTRS,
        };
        let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scatter-mesh-pl"),
            bind_group_layouts: &[Some(&self.bind_group_layout)],
            immediate_size: 0,
        });
        let mesh_depth_stencil = |depth_write_enabled: bool| wgpu::DepthStencilState {
            format: crate::DEPTH_STENCIL_FORMAT,
            depth_write_enabled: Some(depth_write_enabled),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: scatter_scene_stencil_state(),
            bias: wgpu::DepthBiasState::default(),
        };
        let mesh_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        self.mesh_solid_pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("scatter-mesh-solid-opaque"),
                layout: Some(&mesh_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &mesh_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[mesh_vertex_layout.clone()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &mesh_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.surface_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(mesh_depth_stencil(true)),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            },
        ));
        self.mesh_transparent_pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("scatter-mesh-solid-transparent"),
                layout: Some(&mesh_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &mesh_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[mesh_vertex_layout.clone()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &mesh_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.surface_format,
                        blend: Some(mesh_blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(mesh_depth_stencil(false)),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            },
        ));
        self.mesh_wire_pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("scatter-mesh-wire"),
                layout: Some(&mesh_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &mesh_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[mesh_vertex_layout],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &mesh_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.surface_format,
                        blend: Some(mesh_blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: Some(mesh_depth_stencil(false)),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            },
        ));
    }

    pub fn add_mesh_actor(
        &mut self,
        id: u32,
        positions: Vec<[f32; 3]>,
        triangle_indices: Vec<[u32; 3]>,
        color: [f32; 4],
        wireframe: bool,
        device: &wgpu::Device,
    ) {
        self.ensure_mesh_pipelines(device);
        let actor = MeshActor::new(id, positions, triangle_indices, color, wireframe, device);
        self.mesh_actors.insert(id, actor);
        self.mark_scene_dirty();
    }

    pub fn update_mesh_actor(
        &mut self,
        id: u32,
        positions: Option<Vec<[f32; 3]>>,
        triangle_indices: Option<Vec<[u32; 3]>>,
        color: Option<[f32; 4]>,
        wireframe: Option<bool>,
        device: &wgpu::Device,
    ) {
        let Some(actor) = self.mesh_actors.get_mut(&id) else {
            return;
        };
        let mut geometry_changed = false;
        if let Some(p) = positions {
            actor.positions = p;
            geometry_changed = true;
        }
        if let Some(t) = triangle_indices {
            actor.triangle_indices = t;
            geometry_changed = true;
        }
        if let Some(c) = color {
            actor.color = c;
            geometry_changed = true;
        }
        if let Some(w) = wireframe {
            if w != actor.wireframe {
                actor.wireframe = w;
                geometry_changed = true;
            }
        }
        if geometry_changed {
            actor.rebuild_buffers(device);
            self.mark_scene_dirty();
        }
    }

    pub fn remove_mesh_actor(&mut self, id: u32) {
        if self.mesh_actors.remove(&id).is_some() {
            self.mark_scene_dirty();
        }
    }

    pub fn set_mesh_actor_visible(&mut self, id: u32, visible: bool) {
        if let Some(a) = self.mesh_actors.get_mut(&id) {
            if a.visible != visible {
                a.visible = visible;
                self.mark_scene_dirty();
            }
        }
    }

    pub fn clear_mesh_actors(&mut self) {
        if !self.mesh_actors.is_empty() {
            self.mesh_actors.clear();
            self.mark_scene_dirty();
        }
    }

    /// Bounds that include all visible mesh actors.
    pub fn merged_mesh_bounds(&self) -> Option<(glam::Vec3, glam::Vec3)> {
        let mut mn = glam::Vec3::splat(f32::MAX);
        let mut mx = glam::Vec3::splat(f32::MIN);
        let mut any = false;
        for a in self.mesh_actors.values() {
            if a.visible && a.data_min.x <= a.data_max.x {
                mn = mn.min(a.data_min);
                mx = mx.max(a.data_max);
                any = true;
            }
        }
        if any {
            Some((mn, mx))
        } else {
            None
        }
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        let left = self.offset[0];
        let top = self.offset[1];
        let right = left + self.width as f32;
        let bottom = top + self.height as f32;
        let scissor_left = self.scissor_offset[0] as f32;
        let scissor_top = self.scissor_offset[1] as f32;
        let scissor_right = scissor_left + self.scissor_size[0] as f32;
        let scissor_bottom = scissor_top + self.scissor_size[1] as f32;
        if x < scissor_left || x >= scissor_right || y < scissor_top || y >= scissor_bottom {
            return false;
        }
        if x < left || x >= right || y < top || y >= bottom {
            return false;
        }
        rounded_clip_contains(
            x - left,
            y - top,
            self.width as f32,
            self.height as f32,
            self.clip_radii,
        )
    }

    /// Project a single point and return `(dist2, depth)` if it's within `radius_px`.
    pub(crate) fn pick_distance(
        &self,
        point: PointInstance,
        local_x: f32,
        local_y: f32,
        radius_px: f32,
    ) -> Option<(f32, f32)> {
        if !point.position[0].is_finite()
            || !point.position[1].is_finite()
            || !point.position[2].is_finite()
        {
            return None;
        }
        let clip = self.camera.view_proj()
            * glam::Vec4::new(point.position[0], point.position[1], point.position[2], 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 || ndc.z < 0.0 || ndc.z > 1.0 {
            return None;
        }
        let sx = (ndc.x * 0.5 + 0.5) * self.width as f32;
        let sy = (0.5 - ndc.y * 0.5) * self.height as f32;
        let dx = sx - local_x;
        let dy = sy - local_y;
        let threshold = radius_px.max(self.effective_point_size(point.size) * 0.75);
        let dist2 = dx * dx + dy * dy;
        if dist2 > threshold * threshold {
            return None;
        }
        Some((dist2, ndc.z))
    }

    /// Return indices of all points (in `points`) whose screen projection falls
    /// within the pixel rect `[x0, y0, x1, y1]` relative to the scatter viewport.
    pub fn select_points_in_rect(&self, points: &[PointInstance], rect: [f32; 4]) -> Vec<usize> {
        if self.width == 0 || self.height == 0 {
            return Vec::new();
        }
        let [rx0, ry0, rx1, ry1] = [
            rect[0].min(rect[2]),
            rect[1].min(rect[3]),
            rect[0].max(rect[2]),
            rect[1].max(rect[3]),
        ];
        let view_proj = self.camera.view_proj();
        let mut selected = Vec::new();
        for (idx, point) in points.iter().copied().enumerate() {
            if !point.position[0].is_finite()
                || !point.position[1].is_finite()
                || !point.position[2].is_finite()
            {
                continue;
            }
            let clip = view_proj
                * glam::Vec4::new(point.position[0], point.position[1], point.position[2], 1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 || ndc.z < 0.0 || ndc.z > 1.0 {
                continue;
            }
            let sx = (ndc.x * 0.5 + 0.5) * self.width as f32;
            let sy = (0.5 - ndc.y * 0.5) * self.height as f32;
            if sx >= rx0 && sx <= rx1 && sy >= ry0 && sy <= ry1 {
                selected.push(idx);
            }
        }
        selected
    }

    /// Return indices of all points whose screen projection falls inside a polygon.
    ///
    /// `poly` is a list of viewport-local pixel coordinates forming the polygon boundary.
    /// Uses the ray-casting (even-odd) algorithm for point-in-polygon.
    pub fn select_points_in_polygon(
        &self,
        points: &[PointInstance],
        poly: &[[f32; 2]],
    ) -> Vec<usize> {
        if self.width == 0 || self.height == 0 || poly.len() < 3 {
            return Vec::new();
        }
        let view_proj = self.camera.view_proj();
        let mut selected = Vec::new();
        for (idx, point) in points.iter().copied().enumerate() {
            if !point.position[0].is_finite()
                || !point.position[1].is_finite()
                || !point.position[2].is_finite()
            {
                continue;
            }
            let clip = view_proj
                * glam::Vec4::new(point.position[0], point.position[1], point.position[2], 1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            if ndc.x.abs() > 1.0 || ndc.y.abs() > 1.0 || ndc.z < 0.0 || ndc.z > 1.0 {
                continue;
            }
            let sx = (ndc.x * 0.5 + 0.5) * self.width as f32;
            let sy = (0.5 - ndc.y * 0.5) * self.height as f32;
            if point_in_polygon(sx, sy, poly) {
                selected.push(idx);
            }
        }
        selected
    }

    /// Record draw commands into an active render pass.
    ///
    /// Renders grid lines first (behind points), then point billboards,
    /// then 2D screen-space overlays (orientation axes, legend, scalar bar).
    /// Applies viewport and scissor so the scatter only draws within its region.
    pub fn render<'pass, 'data: 'pass>(&'data self, pass: &mut wgpu::RenderPass<'pass>) {
        self.render_with_viewport(
            pass,
            self.offset,
            [self.width as f32, self.height as f32],
            self.scissor_offset,
            self.scissor_size,
        );
    }

    /// Render into the window surface while guaranteeing the strict wgpu
    /// scissor invariant. Fractional layout rounding and a resize can leave a
    /// one-pixel scissor at exactly the old target edge; wgpu treats that as a
    /// validation error rather than an empty draw.
    pub fn render_in_target<'pass, 'data: 'pass>(
        &'data self,
        pass: &mut wgpu::RenderPass<'pass>,
        target_width: u32,
        target_height: u32,
    ) {
        let Some((scissor_offset, scissor_size)) = clamp_scissor_to_target(
            self.scissor_offset,
            self.scissor_size,
            target_width,
            target_height,
        ) else {
            return;
        };
        self.render_with_viewport(
            pass,
            self.offset,
            [self.width as f32, self.height as f32],
            scissor_offset,
            scissor_size,
        );
    }

    pub fn render_offscreen<'pass, 'data: 'pass>(
        &'data self,
        pass: &mut wgpu::RenderPass<'pass>,
        target_width: u32,
        target_height: u32,
    ) {
        let (scissor_offset, scissor_size) = self.scaled_local_scissor(target_width, target_height);
        self.render_with_viewport(
            pass,
            [0.0, 0.0],
            [target_width as f32, target_height as f32],
            scissor_offset,
            scissor_size,
        );
    }

    pub fn scaled_render_target_size(&self, scale: f32) -> (u32, u32) {
        let scale = clamp_interactive_render_scale(scale);
        (
            ((self.width as f32) * scale).ceil().max(1.0) as u32,
            ((self.height as f32) * scale).ceil().max(1.0) as u32,
        )
    }

    fn scaled_local_scissor(&self, target_width: u32, target_height: u32) -> ([u32; 2], [u32; 2]) {
        if self.width == 0 || self.height == 0 || target_width == 0 || target_height == 0 {
            return ([0, 0], [0, 0]);
        }
        let sx = target_width as f32 / self.width as f32;
        let sy = target_height as f32 / self.height as f32;
        let left = ((self.scissor_offset[0] as f32 - self.offset[0]).max(0.0) * sx)
            .floor()
            .clamp(0.0, target_width as f32) as u32;
        let top = ((self.scissor_offset[1] as f32 - self.offset[1]).max(0.0) * sy)
            .floor()
            .clamp(0.0, target_height as f32) as u32;
        let right = ((self.scissor_offset[0] as f32 + self.scissor_size[0] as f32 - self.offset[0])
            .max(0.0)
            * sx)
            .ceil()
            .clamp(left as f32, target_width as f32) as u32;
        let bottom = ((self.scissor_offset[1] as f32 + self.scissor_size[1] as f32
            - self.offset[1])
            .max(0.0)
            * sy)
            .ceil()
            .clamp(top as f32, target_height as f32) as u32;
        (
            [left, top],
            [right.saturating_sub(left), bottom.saturating_sub(top)],
        )
    }

    fn render_with_viewport<'pass, 'data: 'pass>(
        &'data self,
        pass: &mut wgpu::RenderPass<'pass>,
        viewport_offset: [f32; 2],
        viewport_size: [f32; 2],
        scissor_offset: [u32; 2],
        scissor_size: [u32; 2],
    ) {
        let has_grid = self.chrome.grid_visible
            && self.grid_vertex_count > 0
            && self.line_vertex_buffer.is_some();
        let has_user_lines =
            self.user_line_vertex_count > 0 && self.user_line_vertex_buffer.is_some();
        let has_points = self.point_count > 0 && self.primary_first_buffer().is_some();
        let has_extra_points = self
            .extra_actors
            .values()
            .any(|actor| actor.visible && actor.point_count > 0 && actor.vertex_buffer.is_some());
        let has_overlay = self.overlay_vertex_count > 0 && self.overlay_vertex_buffer.is_some();
        let has_bg = self.bg_vertex_count > 0 && self.bg_vertex_buffer.is_some();

        if (!has_bg
            && !has_grid
            && !has_user_lines
            && !has_points
            && !has_extra_points
            && !has_overlay)
            || self.width == 0
            || self.height == 0
            || scissor_size[0] == 0
            || scissor_size[1] == 0
        {
            return;
        }

        pass.set_viewport(
            viewport_offset[0],
            viewport_offset[1],
            viewport_size[0],
            viewport_size[1],
            0.0,
            1.0,
        );
        pass.set_scissor_rect(
            scissor_offset[0],
            scissor_offset[1],
            scissor_size[0],
            scissor_size[1],
        );
        pass.set_stencil_reference(1);
        let Some(clip_mask_pipeline) = self.clip_mask_pipeline.as_ref() else {
            return;
        };
        pass.set_pipeline(clip_mask_pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..4, 0..1);

        // Background fill drawn first so everything else renders on top.
        if has_bg {
            if let (Some(pipeline), Some(buffer)) =
                (self.bg_pipeline.as_ref(), self.bg_vertex_buffer.as_ref())
            {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..self.bg_vertex_count, 0..1);
            }
        }

        if has_grid {
            if let (Some(pipeline), Some(buffer)) = (
                self.line_pipeline.as_ref(),
                self.line_vertex_buffer.as_ref(),
            ) {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..self.grid_vertex_count, 0..1);
            }
        }

        if has_user_lines {
            if let (Some(pipeline), Some(buffer)) = (
                self.line_pipeline.as_ref(),
                self.user_line_vertex_buffer.as_ref(),
            ) {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..self.user_line_vertex_count, 0..1);
            }
        }

        let flat_2d = !scatter_points_write_depth(self.interaction_mode);
        let primary_view_proj = self.camera.view_proj();
        let mut draw_point_buffer =
            |_opaque: bool, compact: bool, vb: &wgpu::Buffer, draw_count: u32| {
                let pipeline = if compact {
                    if self.primary_storage == PrimaryPointStorage::XyF32 {
                        let Some(pipeline) = self.compact_xy_pipeline.as_ref() else {
                            return;
                        };
                        pipeline
                    } else if flat_2d {
                        let Some(pipeline) = self.compact_pipeline.as_ref() else {
                            return;
                        };
                        pipeline
                    } else {
                        let Some(pipeline) = self.compact_opaque_pipeline.as_ref() else {
                            return;
                        };
                        pipeline
                    }
                } else if !flat_2d {
                    let Some(pipeline) = self.opaque_pipeline.as_ref() else {
                        return;
                    };
                    pipeline
                } else {
                    &self.pipeline
                };
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.draw(0..4, 0..draw_count);
            };

        if has_points && self.primary_points_opaque && !flat_2d {
            let is_lod =
                self.lod_enabled && self.lod_active && self.point_count > self.lod_threshold;
            // Use the pre-shuffled LOD buffer when active so the first N instances
            // are a representative spatial sample rather than a positional prefix.
            let draw_vb = if is_lod {
                self.lod_vertex_buffer.as_ref()
            } else {
                self.primary_first_buffer()
            };
            if let Some(vb) = draw_vb {
                let draw_count = if is_lod {
                    lod_sample_count(self.point_count as usize, self.lod_factor) as u32
                } else {
                    self.primary_draw_counts
                        .first()
                        .copied()
                        .unwrap_or(self.point_count)
                };
                let compact = self.primary_storage != PrimaryPointStorage::PointInstance && !is_lod;
                if is_lod || self.primary_chunk_visible_with_view_proj(0, primary_view_proj) {
                    draw_point_buffer(true, compact, vb, draw_count);
                }
                if compact && self.primary_chunk_count() > 1 {
                    if let Some(shared) = &self.shared_primary {
                        for (index, (buffer, draw_count)) in shared
                            .buffers
                            .iter()
                            .skip(1)
                            .zip(self.primary_draw_counts.iter().skip(1).copied())
                            .enumerate()
                        {
                            if self
                                .primary_chunk_visible_with_view_proj(index + 1, primary_view_proj)
                            {
                                draw_point_buffer(true, true, buffer, draw_count);
                            }
                        }
                    } else {
                        for (index, (buffer, draw_count)) in self
                            .primary_extra_buffers
                            .iter()
                            .zip(self.primary_draw_counts.iter().skip(1).copied())
                            .enumerate()
                        {
                            if self
                                .primary_chunk_visible_with_view_proj(index + 1, primary_view_proj)
                            {
                                draw_point_buffer(true, true, buffer, draw_count);
                            }
                        }
                    }
                }
            }
        }

        // Draw opaque actors before translucent point sets so their depth can
        // occlude later transparent billboard fragments correctly.
        for actor in self.extra_actors.values() {
            if actor.visible && actor.point_count > 0 && actor.points_opaque && !flat_2d {
                let is_lod =
                    self.lod_enabled && self.lod_active && actor.point_count > self.lod_threshold;
                let draw_vb = if is_lod {
                    actor
                        .lod_vertex_buffer
                        .as_ref()
                        .or(actor.vertex_buffer.as_ref())
                } else {
                    actor.vertex_buffer.as_ref()
                };
                if let Some(vb) = draw_vb {
                    let draw_count = if is_lod {
                        lod_sample_count(actor.point_count as usize, self.lod_factor) as u32
                    } else {
                        actor.point_count
                    };
                    draw_point_buffer(true, false, vb, draw_count);
                }
            }
        }

        if has_points && (!self.primary_points_opaque || flat_2d) {
            let is_lod =
                self.lod_enabled && self.lod_active && self.point_count > self.lod_threshold;
            let draw_vb = if is_lod {
                self.lod_vertex_buffer
                    .as_ref()
                    .or(self.primary_first_buffer())
            } else {
                self.primary_first_buffer()
            };
            if let Some(vb) = draw_vb {
                let draw_count = if is_lod {
                    lod_sample_count(self.point_count as usize, self.lod_factor) as u32
                } else if self.primary_storage != PrimaryPointStorage::PointInstance {
                    self.primary_draw_counts
                        .first()
                        .copied()
                        .unwrap_or(self.point_count)
                } else {
                    self.point_count
                };
                let compact = self.primary_storage != PrimaryPointStorage::PointInstance && !is_lod;
                if is_lod || self.primary_chunk_visible_with_view_proj(0, primary_view_proj) {
                    draw_point_buffer(false, compact, vb, draw_count);
                }
                if compact && self.primary_chunk_count() > 1 {
                    if let Some(shared) = &self.shared_primary {
                        for (index, (buffer, draw_count)) in shared
                            .buffers
                            .iter()
                            .skip(1)
                            .zip(self.primary_draw_counts.iter().skip(1).copied())
                            .enumerate()
                        {
                            if self
                                .primary_chunk_visible_with_view_proj(index + 1, primary_view_proj)
                            {
                                draw_point_buffer(false, true, buffer, draw_count);
                            }
                        }
                    } else {
                        for (index, (buffer, draw_count)) in self
                            .primary_extra_buffers
                            .iter()
                            .zip(self.primary_draw_counts.iter().skip(1).copied())
                            .enumerate()
                        {
                            if self
                                .primary_chunk_visible_with_view_proj(index + 1, primary_view_proj)
                            {
                                draw_point_buffer(false, true, buffer, draw_count);
                            }
                        }
                    }
                }
            }
        }

        for actor in self.extra_actors.values() {
            if actor.visible && actor.point_count > 0 && (!actor.points_opaque || flat_2d) {
                let is_lod =
                    self.lod_enabled && self.lod_active && actor.point_count > self.lod_threshold;
                let draw_vb = if is_lod {
                    actor
                        .lod_vertex_buffer
                        .as_ref()
                        .or(actor.vertex_buffer.as_ref())
                } else {
                    actor.vertex_buffer.as_ref()
                };
                if let Some(vb) = draw_vb {
                    let draw_count = if is_lod {
                        lod_sample_count(actor.point_count as usize, self.lod_factor) as u32
                    } else {
                        actor.point_count
                    };
                    draw_point_buffer(false, false, vb, draw_count);
                }
            }
        }

        // Mesh overlays follow DragonSci's order: wireframes first, then opaque
        // fills with depth writes, then translucent fills sorted back-to-front.
        let view_proj = self.camera.view_proj();
        let mesh_depth = |actor: &MeshActor| -> f32 {
            let center = (actor.data_min + actor.data_max) * 0.5;
            let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
            if clip.w.abs() > 1e-7 {
                clip.z / clip.w
            } else {
                0.0
            }
        };

        let mut opaque_meshes = Vec::new();
        let mut transparent_meshes = Vec::new();
        let mut wire_meshes = Vec::new();
        for actor in self
            .mesh_actors
            .values()
            .filter(|a| a.visible && a.index_count > 0)
        {
            if actor.wireframe {
                wire_meshes.push((actor, mesh_depth(actor)));
            } else if actor.color[3] >= 1.0 {
                opaque_meshes.push(actor);
            } else {
                transparent_meshes.push((actor, mesh_depth(actor)));
            }
        }

        wire_meshes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if !wire_meshes.is_empty() {
            let Some(pipeline) = self.mesh_wire_pipeline.as_ref() else {
                return;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for (actor, _) in wire_meshes {
                actor.render_into(pass);
            }
        }

        if !opaque_meshes.is_empty() {
            let Some(pipeline) = self.mesh_solid_pipeline.as_ref() else {
                return;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for actor in opaque_meshes {
                actor.render_into(pass);
            }
        }

        transparent_meshes
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if !transparent_meshes.is_empty() {
            let Some(pipeline) = self.mesh_transparent_pipeline.as_ref() else {
                return;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for (actor, _) in transparent_meshes {
                actor.render_into(pass);
            }
        }

        if has_overlay {
            if let (Some(pipeline), Some(buffer)) = (
                self.overlay_pipeline.as_ref(),
                self.overlay_vertex_buffer.as_ref(),
            ) {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..self.overlay_vertex_count, 0..1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scatter_buffer_growth_is_clamped_to_device_limit() {
        let mib = 1024 * 1024;
        assert_eq!(
            bounded_scatter_buffer_capacity(160 * mib, 256 * mib),
            Some(256 * mib)
        );
        assert_eq!(
            bounded_scatter_buffer_capacity(64 * mib, 256 * mib),
            Some(128 * mib)
        );
    }

    #[test]
    fn scatter_buffer_capacity_rejects_uploads_larger_than_device_limit() {
        let mib = 1024 * 1024;
        let limit = 256 * mib;
        assert_eq!(
            bounded_scatter_buffer_capacity(limit - 1, limit),
            Some(limit)
        );
        assert_eq!(bounded_scatter_buffer_capacity(limit, limit), Some(limit));
        assert_eq!(bounded_scatter_buffer_capacity(limit + 1, limit), None);
        assert_eq!(bounded_scatter_buffer_capacity(0, limit), None);
    }

    #[test]
    fn xy_chunk_layout_crosses_single_buffer_boundary_without_splitting_points() {
        let limit = 256 * 1024 * 1024;
        assert_eq!(scatter_chunk_layout(limit, limit, 8), Some((limit, 1)));
        assert_eq!(scatter_chunk_layout(limit + 8, limit, 8), Some((limit, 2)));
        assert_eq!(
            scatter_chunk_layout(limit * 2 + 8, limit, 8),
            Some((limit, 3))
        );
    }

    #[test]
    fn xy_chunk_layout_aligns_odd_device_limits_and_rejects_partial_points() {
        assert_eq!(scatter_chunk_layout(32, 19, 8), Some((16, 2)));
        assert_eq!(scatter_chunk_layout(18, 19, 8), None);
        assert_eq!(scatter_chunk_layout(8, 7, 8), None);
    }

    #[test]
    fn xyz_chunk_layout_crosses_limit_without_splitting_points() {
        let limit = 256 * 1024 * 1024;
        let aligned = limit / 12 * 12;
        assert_eq!(scatter_chunk_layout(aligned, limit, 12), Some((aligned, 1)));
        assert_eq!(
            scatter_chunk_layout(aligned + 12, limit, 12),
            Some((aligned, 2))
        );
        assert_eq!(scatter_chunk_layout(24, 11, 12), None);
    }

    #[test]
    fn compact_xyz_bounds_ignore_nonfinite_rows() {
        let values = [1.0_f32, 2.0, 3.0, f32::NAN, 10.0, 10.0, -4.0, 5.0, 2.0];
        let (min, max) = compact_xyz_chunk_bounds(bytemuck::cast_slice(&values)).unwrap();
        assert_eq!(min, glam::Vec3::new(-4.0, 2.0, 2.0));
        assert_eq!(max, glam::Vec3::new(1.0, 5.0, 3.0));
    }

    #[test]
    fn aabb_clip_test_is_conservative_for_inside_outside_and_crossing_bounds() {
        let identity = glam::Mat4::IDENTITY;
        assert!(aabb_intersects_clip(
            (
                glam::Vec3::new(-0.5, -0.5, 0.1),
                glam::Vec3::new(0.5, 0.5, 0.9)
            ),
            identity,
        ));
        assert!(!aabb_intersects_clip(
            (
                glam::Vec3::new(2.0, -0.5, 0.1),
                glam::Vec3::new(3.0, 0.5, 0.9)
            ),
            identity,
        ));
        assert!(aabb_intersects_clip(
            (
                glam::Vec3::new(0.5, -0.5, 0.1),
                glam::Vec3::new(2.0, 0.5, 0.9)
            ),
            identity,
        ));
    }

    #[test]
    fn scatter_capacity_error_reports_actionable_limit_context() {
        let error = scatter_capacity_error(120_000_012, 120_000_000, 12, "compact_xyz_f32_v0");
        assert_eq!(error.requested_points, 10_000_001);
        assert_eq!(error.required_bytes, 120_000_012);
        assert_eq!(error.max_buffer_size, 120_000_000);
        assert_eq!(error.stride_bytes, 12);
        assert_eq!(error.representation, "compact_xyz_f32_v0");
        let message = error.to_string();
        assert!(message.contains("10000001 points"));
        assert!(message.contains("120000012 bytes"));
        assert!(message.contains("max_buffer_size is 120000000 bytes"));
        assert!(message.contains("rendering='adaptive'"));
    }

    #[test]
    fn point_size_override_uses_negative_sentinel_for_default() {
        assert_eq!(point_size_override_value(None), -1.0);
        assert_eq!(point_size_override_value(Some(6.0)), 6.0);
        assert_eq!(point_size_override_value(Some(-2.0)), 0.0);
    }

    #[test]
    fn point_opacity_detection_treats_default_alpha_as_opaque() {
        let opaque = [make_pt(0.0, 0.0, 0.0), make_pt(1.0, 1.0, 1.0)];
        assert!(points_are_opaque(&opaque));

        let mut transparent = opaque;
        transparent[1].alpha = 0.998;
        assert!(!points_are_opaque(&transparent));
    }

    #[test]
    fn point_instance_bytes_opacity_detection_reads_alpha_slot() {
        let mut opaque = [make_pt(0.0, 0.0, 0.0), make_pt(1.0, 1.0, 1.0)];
        assert!(point_instance_bytes_are_opaque(bytemuck::cast_slice(
            &opaque
        )));

        opaque[0].alpha = 0.5;
        assert!(!point_instance_bytes_are_opaque(bytemuck::cast_slice(
            &opaque
        )));
    }

    #[test]
    fn adaptive_point_size_scale_shrinks_dense_views() {
        assert_eq!(adaptive_point_size_scale(0, 800, 600), 1.0);
        assert_eq!(adaptive_point_size_scale(10_000, 800, 600), 1.0);
        assert!(adaptive_point_size_scale(125_000, 800, 600) < 1.0);
        assert!(adaptive_point_size_scale(1_000_000, 800, 600) <= 0.35);
    }

    #[test]
    fn zoom_out_point_size_scale_shrinks_far_views() {
        let fit = fitted_camera_distance(10.0, 1.0, 45_f32.to_radians());
        assert!(fit > 0.0);
        assert_eq!(zoom_out_point_size_scale(fit, fit), 1.0);
        assert!(zoom_out_point_size_scale(fit * 3.0, fit) < 1.0);
        assert!(zoom_out_point_size_scale(fit * 8.0, fit) <= 0.45);
    }

    #[test]
    fn pan2d_cursor_anchor_adjustment_preserves_off_center_data_point() {
        let before = [-10.0, -5.0, 10.0, 5.0];
        let after = [-5.0, -2.5, 5.0, 2.5];
        let fraction = [0.8, 0.2];
        let adjustment = pan2d_cursor_anchor_adjustment(before, after, fraction);

        assert!((adjustment.x - 3.0).abs() < 0.0001);
        assert!((adjustment.y - 1.5).abs() < 0.0001);
        let anchored_after = glam::Vec2::new(
            after[0] + adjustment.x + (after[2] - after[0]) * fraction[0],
            after[1] + adjustment.y + (after[3] - after[1]) * (1.0 - fraction[1]),
        );
        assert!((anchored_after.x - 6.0).abs() < 0.0001);
        assert!((anchored_after.y - 3.0).abs() < 0.0001);
    }

    #[test]
    fn clipped_scatter_keeps_full_viewport_and_visible_scissor() {
        let rect = scatter_layout_rect(20.0, 40.0, 300.0, 180.0, Some([20.0, 96.0, 300.0, 72.0]));

        assert_eq!(rect.offset, [20.0, 40.0]);
        assert_eq!(rect.width, 300);
        assert_eq!(rect.height, 180);
        assert_eq!(rect.scissor_offset, [20, 96]);
        assert_eq!(rect.scissor_size, [300, 72]);
        assert_eq!(
            scatter_scissor_bounds(rect.scissor_offset, rect.scissor_size),
            [20.0, 96.0, 320.0, 168.0]
        );
    }

    #[test]
    fn orbit_3d_points_always_write_depth_but_flat_2d_points_do_not() {
        assert!(scatter_points_write_depth(ScatterInteractionMode::Orbit3D));
        assert!(!scatter_points_write_depth(ScatterInteractionMode::Pan2D));
    }

    #[test]
    fn surface_scissor_is_clamped_inside_resized_render_target() {
        assert_eq!(
            clamp_scissor_to_target([1021, 1079], [376, 4], 1848, 1080),
            Some(([1021, 1079], [376, 1]))
        );
        assert_eq!(
            clamp_scissor_to_target([1021, 1080], [376, 1], 1848, 1080),
            None
        );
        assert_eq!(
            clamp_scissor_to_target([1847, 40], [20, 30], 1848, 1080),
            Some(([1847, 40], [1, 30]))
        );
    }

    #[test]
    fn projected_axis_label_rect_is_centered_on_its_tick_relative_anchor() {
        let size = estimate_scatter_label_size("-12.50", false);
        let rect = scatter_label_rect(140.0, 90.0, "-12.50", false);

        const TOLERANCE: f32 = 0.001;
        assert!(((rect[0] + rect[2]) * 0.5 - 140.0).abs() < TOLERANCE);
        assert!(((rect[1] + rect[3]) * 0.5 - 90.0).abs() < TOLERANCE);
        assert!((rect[2] - rect[0] - size.x).abs() < TOLERANCE);
        assert!((rect[3] - rect[1] - size.y).abs() < TOLERANCE);
    }

    #[test]
    fn scatter_uniform_layout_stays_wgpu_aligned() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 272);
    }

    #[test]
    fn rounded_clip_contains_respects_corner_radii() {
        assert!(rounded_clip_contains(10.0, 10.0, 100.0, 60.0, [0.0; 4]));
        assert!(!rounded_clip_contains(
            2.0,
            2.0,
            100.0,
            60.0,
            [20.0, 0.0, 0.0, 0.0]
        ));
        assert!(rounded_clip_contains(
            18.0,
            18.0,
            100.0,
            60.0,
            [20.0, 0.0, 0.0, 0.0]
        ));
    }

    fn make_pt(x: f32, y: f32, z: f32) -> PointInstance {
        PointInstance {
            position: [x, y, z],
            size: 4.0,
            color: [1.0, 0.0, 0.0],
            alpha: 1.0,
        }
    }

    // With the identity view_proj, position == clip (w=1), so:
    //   sx = (pos.x * 0.5 + 0.5) * W
    //   sy = (0.5 - pos.y * 0.5) * H
    // For a 100×100 viewport:
    //   pos (0,0,0.5) → screen (50, 50)
    //   pos (-0.8, 0.8, 0.5) → screen (10, 10)
    //   pos ( 0.8,-0.8, 0.5) → screen (90, 90)

    #[test]
    fn screen_pick_cache_empty_points_builds_cleanly() {
        let vp = glam::Mat4::IDENTITY;
        let cache = build_cache(&[], vp, 200, 100);
        assert_eq!(cache.sorted_indices.len(), 0);
        assert!(cache.candidates(100.0, 50.0, 20.0).is_empty());
    }

    #[test]
    fn screen_pick_cache_is_stale_on_size_change() {
        let vp = glam::Mat4::IDENTITY;
        let cache = build_cache(&[], vp, 200, 100);
        assert!(!cache.is_stale(&vp, 200, 100, -1.0));
        assert!(cache.is_stale(&vp, 201, 100, -1.0));
        assert!(cache.is_stale(&vp, 200, 101, -1.0));
    }

    #[test]
    fn screen_pick_cache_is_stale_on_point_size_override_change() {
        let vp = glam::Mat4::IDENTITY;
        let pt = PointInstance {
            position: [0.0, 0.0, 0.5],
            size: 4.0,
            color: [1.0, 0.0, 0.0],
            alpha: 1.0,
        };
        let cache = ScreenPickCache::build(&[pt], vp, 100, 100, -1.0);
        assert!(
            !cache.is_stale(&vp, 100, 100, -1.0),
            "same override must not be stale"
        );
        assert!(
            cache.is_stale(&vp, 100, 100, 80.0),
            "changed override must mark stale"
        );
    }

    // Helper: build cache with default point_size_override (-1.0 = per-point).
    fn build_cache(points: &[PointInstance], vp: glam::Mat4, w: u32, h: u32) -> ScreenPickCache {
        ScreenPickCache::build(points, vp, w, h, -1.0)
    }

    #[test]
    fn screen_pick_cache_is_stale_on_vp_change() {
        let vp = glam::Mat4::IDENTITY;
        let cache = build_cache(&[], vp, 200, 100);
        let vp2 = glam::Mat4::from_scale(glam::Vec3::splat(0.5));
        assert!(cache.is_stale(&vp2, 200, 100, -1.0));
    }

    #[test]
    fn screen_pick_cache_visible_point_appears_in_correct_cell() {
        // pos (0,0,0.5) → screen (50,50) in 100×100 viewport.
        let vp = glam::Mat4::IDENTITY;
        let pt = make_pt(0.0, 0.0, 0.5);
        let cache = build_cache(&[pt], vp, 100, 100);
        assert!((cache._screen_xy[0][0] - 50.0).abs() < 0.01);
        assert!((cache._screen_xy[0][1] - 50.0).abs() < 0.01);
        // Query should include this point.
        let cands = cache.candidates(50.0, 50.0, 5.0);
        assert!(
            cands.contains(&0),
            "visible point must be a candidate near its screen position"
        );
    }

    #[test]
    fn screen_pick_cache_clipped_point_absent() {
        // pos (0,0,-2) → ndc.z = -2, outside frustum → clipped.
        let vp = glam::Mat4::IDENTITY;
        let pt = make_pt(0.0, 0.0, -2.0);
        let cache = build_cache(&[pt], vp, 100, 100);
        assert!(
            cache._screen_xy[0][0].is_nan(),
            "clipped point must have NaN screen_x"
        );
        assert_eq!(cache.sorted_indices.len(), 0, "no visible points in index");
        assert!(cache.candidates(50.0, 50.0, 50.0).is_empty());
    }

    #[test]
    fn screen_pick_cache_candidates_separated_by_cells() {
        // p0 → screen (10,10), p1 → screen (90,90) in 100×100 viewport.
        let vp = glam::Mat4::IDENTITY;
        let p0 = make_pt(-0.8, 0.8, 0.5);
        let p1 = make_pt(0.8, -0.8, 0.5);
        let cache = build_cache(&[p0, p1], vp, 100, 100);

        let c0 = cache.candidates(10.0, 10.0, 5.0);
        assert!(c0.contains(&0), "p0 must be candidate near (10,10)");
        assert!(!c0.contains(&1), "p1 must not be candidate near (10,10)");

        let c1 = cache.candidates(90.0, 90.0, 5.0);
        assert!(c1.contains(&1), "p1 must be candidate near (90,90)");
        assert!(!c1.contains(&0), "p0 must not be candidate near (90,90)");
    }

    #[test]
    fn screen_pick_cache_wide_radius_covers_both_points() {
        let vp = glam::Mat4::IDENTITY;
        let p0 = make_pt(-0.8, 0.8, 0.5);
        let p1 = make_pt(0.8, -0.8, 0.5);
        let cache = build_cache(&[p0, p1], vp, 100, 100);
        // A radius of 100px from center (50,50) covers the whole viewport.
        let cands = cache.candidates(50.0, 50.0, 100.0);
        assert!(cands.contains(&0));
        assert!(cands.contains(&1));
    }

    #[test]
    fn screen_pick_cache_nan_position_not_visible() {
        let vp = glam::Mat4::IDENTITY;
        let pt = make_pt(f32::NAN, 0.0, 0.5);
        let cache = build_cache(&[pt], vp, 100, 100);
        assert!(cache.sorted_indices.is_empty());
    }

    // ── Fix 1: cache invalidation on actor replace ────────────────────────────

    #[test]
    fn actor_replacement_at_same_id_must_use_fresh_cache() {
        // Two caches representing an actor before and after being replaced.
        // The new cache must give different candidates than the old one.
        let vp = glam::Mat4::IDENTITY;
        // Original actor: point at screen (10, 10).
        let old_pt = make_pt(-0.8, 0.8, 0.5);
        let old_cache = build_cache(&[old_pt], vp, 100, 100);
        // Actor replaced: point now at screen (90, 90).
        let new_pt = make_pt(0.8, -0.8, 0.5);
        let new_cache = build_cache(&[new_pt], vp, 100, 100);

        // Old cache finds the original position, new cache finds the new one.
        assert!(old_cache.candidates(10.0, 10.0, 5.0).contains(&0));
        assert!(
            !old_cache.candidates(90.0, 90.0, 5.0).contains(&0),
            "old cache must NOT match replaced position"
        );
        assert!(
            !new_cache.candidates(10.0, 10.0, 5.0).contains(&0),
            "new cache must NOT match old position"
        );
        assert!(
            new_cache.candidates(90.0, 90.0, 5.0).contains(&0),
            "new cache must match replaced position"
        );
    }

    // ── Fix 2: large-point expanded candidate radius ──────────────────────────

    #[test]
    fn screen_pick_cache_large_point_in_expanded_radius() {
        // Point at screen (50, 50), visual size 80px.
        // Cursor at screen (90, 50) → 40px from center.
        // Base radius = 8px, effective threshold = max(8, 80*0.75) = 60px.
        // With base_radius=8 the cell at (90,50) is outside; with 60px it's inside.
        let vp = glam::Mat4::IDENTITY;
        let pt = PointInstance {
            position: [0.0, 0.0, 0.5],
            size: 80.0,
            color: [1.0, 0.0, 0.0],
            alpha: 1.0,
        };
        let cache = build_cache(&[pt], vp, 100, 100);
        assert!(
            (cache.max_point_size - 80.0).abs() < 0.01,
            "max_point_size must be 80"
        );

        let base_radius = 8.0_f32;
        // Narrow query: cursor at (90, 50), radius 8 — too small to reach cell of (50,50).
        let narrow = cache.candidates(90.0, 50.0, base_radius);
        assert!(
            !narrow.contains(&0),
            "base radius must not reach center cell from (90,50)"
        );

        // Expanded query: radius = max(8, 80*0.75) = 60.
        let expanded = base_radius.max(cache.max_point_size * 0.75);
        let wide = cache.candidates(90.0, 50.0, expanded);
        assert!(
            wide.contains(&0),
            "expanded radius must include large-point center cell"
        );
    }

    #[test]
    fn screen_pick_cache_max_point_size_uses_override() {
        // When point_size_override >= 0, max_point_size should be the override, not per-point.
        let vp = glam::Mat4::IDENTITY;
        let pt = PointInstance {
            position: [0.0, 0.0, 0.5],
            size: 4.0,
            color: [1.0, 0.0, 0.0],
            alpha: 1.0,
        };
        let cache_default = ScreenPickCache::build(&[pt], vp, 100, 100, -1.0);
        assert!((cache_default.max_point_size - 4.0).abs() < 0.01);

        let cache_override = ScreenPickCache::build(&[pt], vp, 100, 100, 20.0);
        assert!((cache_override.max_point_size - 20.0).abs() < 0.01);
    }

    // ── Fix 3: depth tie-breaking for same-position points ───────────────────

    #[test]
    fn screen_pick_cache_same_screen_position_both_are_candidates() {
        // Two points at the same screen position (50, 50) but different depths.
        // Both must appear as candidates so pick_distance can select by depth.
        let vp = glam::Mat4::IDENTITY;
        let near = make_pt(0.0, 0.0, 0.2);
        let far = make_pt(0.0, 0.0, 0.8);
        let cache = build_cache(&[near, far], vp, 100, 100);

        let cands = cache.candidates(50.0, 50.0, 5.0);
        assert!(cands.contains(&0), "nearer point must be a candidate");
        assert!(cands.contains(&1), "farther point must be a candidate");
    }

    // ── format_scalar_bar_tick ────────────────────────────────────────────────

    #[test]
    fn scalar_bar_tick_linear_three_decimal_places() {
        assert_eq!(format_scalar_bar_tick(1.0, false), "1.000");
        assert_eq!(format_scalar_bar_tick(0.0, false), "0.000");
        assert_eq!(format_scalar_bar_tick(-3.14159, false), "-3.142");
        assert_eq!(format_scalar_bar_tick(1234.5, false), "1234.500");
    }

    #[test]
    fn scalar_bar_geometry_reserves_title_gap_and_tick_gutter() {
        let width = 640.0;
        let (_x1, x2, top, height) = scalar_bar_geometry(width, 480.0);
        let title_line_bottom = SCALAR_BAR_TITLE_TOP_PX + 13.0 * 1.3;

        assert!(
            top - title_line_bottom >= 8.0,
            "scalar-bar title should have a visible gap before the bar"
        );
        assert_eq!(width - x2, SCALAR_BAR_RIGHT_GUTTER_PX);
        assert!(
            SCALAR_BAR_TICK_LABEL_WIDTH_PX >= 56.0,
            "scalar-bar ticks need room for formatted values before the right inset"
        );
        assert!(
            SCALAR_BAR_RIGHT_GUTTER_PX - SCALAR_BAR_TICK_GAP_PX - SCALAR_BAR_TICK_LABEL_WIDTH_PX
                >= SCALAR_BAR_RIGHT_PADDING_PX,
            "scalar-bar tick labels should retain explicit right-edge padding"
        );
        assert!((60.0..=220.0).contains(&height));
    }

    #[test]
    fn scalar_bar_tick_log_scale_formats_raw_value_as_scientific() {
        // log_scale=true: value is raw domain; display in scientific notation.
        assert_eq!(format_scalar_bar_tick(100.0, true), "1.00e2");
        assert_eq!(format_scalar_bar_tick(1.0, true), "1.00e0");
        assert_eq!(format_scalar_bar_tick(1000.0, true), "1.00e3");
    }

    #[test]
    fn scalar_bar_tick_log_scale_small_raw_value() {
        // 0.001 raw → "1.00e-3"
        assert_eq!(format_scalar_bar_tick(0.001, true), "1.00e-3");
    }

    #[test]
    fn scalar_bar_tick_linear_zero_and_negative() {
        assert_eq!(format_scalar_bar_tick(0.0, false), "0.000");
        assert_eq!(format_scalar_bar_tick(-1.5, false), "-1.500");
    }

    // ── scalar_bar_tick_values ────────────────────────────────────────────────

    #[test]
    fn tick_values_linear_endpoints() {
        let ticks = scalar_bar_tick_values(0.0, 10.0, false, 3);
        assert_eq!(ticks.len(), 3);
        assert!((ticks[0] - 0.0).abs() < 1e-5);
        assert!((ticks[1] - 5.0).abs() < 1e-5);
        assert!((ticks[2] - 10.0).abs() < 1e-5);
    }

    #[test]
    fn tick_values_linear_two_returns_endpoints_only() {
        let ticks = scalar_bar_tick_values(-5.0, 5.0, false, 2);
        assert_eq!(ticks.len(), 2);
        assert!((ticks[0] - -5.0).abs() < 1e-5);
        assert!((ticks[1] - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_values_equal_vmin_vmax_returns_single_tick() {
        // Equal range: return one tick so the scalar bar shows the constant value.
        let ticks = scalar_bar_tick_values(3.0, 3.0, false, 5);
        assert_eq!(ticks, vec![3.0]);
        let ticks_log = scalar_bar_tick_values(3.0, 3.0, true, 5);
        assert_eq!(ticks_log, vec![3.0]);
    }

    #[test]
    fn tick_values_count_less_than_two_returns_empty() {
        assert!(scalar_bar_tick_values(0.0, 1.0, false, 0).is_empty());
        assert!(scalar_bar_tick_values(0.0, 1.0, false, 1).is_empty());
    }

    #[test]
    fn tick_values_log_endpoints_are_raw_domain() {
        // 1..1000 log-spaced: endpoints must be 1 and 1000 exactly.
        let ticks = scalar_bar_tick_values(1.0, 1000.0, true, 4);
        assert_eq!(ticks.len(), 4);
        assert!((ticks[0] - 1.0).abs() < 1e-3);
        assert!((ticks[3] - 1000.0).abs() < 1.0);
    }

    #[test]
    fn tick_values_log_middle_tick_is_geometric_mean() {
        // 1..100 log-spaced with 3 ticks: middle should be sqrt(1*100) = 10.
        let ticks = scalar_bar_tick_values(1.0, 100.0, true, 3);
        assert_eq!(ticks.len(), 3);
        assert!((ticks[1] - 10.0).abs() < 0.1);
    }

    #[test]
    fn tick_values_log_non_positive_falls_back_to_linear() {
        // When vmin <= 0, log path is unavailable; falls back to linear.
        let ticks = scalar_bar_tick_values(-10.0, 10.0, true, 3);
        assert_eq!(ticks.len(), 3);
        assert!((ticks[0] - -10.0).abs() < 1e-5);
        assert!((ticks[1] - 0.0).abs() < 1e-5);
        assert!((ticks[2] - 10.0).abs() < 1e-5);
    }
}

fn clamp_clip_radii(radii: [f32; 4], width: f32, height: f32) -> [f32; 4] {
    let limit = width.min(height).max(0.0) * 0.5;
    radii.map(|radius| radius.max(0.0).min(limit))
}

fn rounded_clip_contains(
    local_x: f32,
    local_y: f32,
    width: f32,
    height: f32,
    radii: [f32; 4],
) -> bool {
    if local_x < 0.0 || local_y < 0.0 || local_x >= width || local_y >= height {
        return false;
    }
    let max_radius = radii.iter().copied().fold(0.0_f32, f32::max);
    if max_radius <= f32::EPSILON {
        return true;
    }
    let cx = local_x - width * 0.5;
    let cy = local_y - height * 0.5;
    let radius = if cy < 0.0 {
        if cx < 0.0 {
            radii[0]
        } else {
            radii[1]
        }
    } else if cx < 0.0 {
        radii[3]
    } else {
        radii[2]
    };
    if radius <= f32::EPSILON {
        return true;
    }
    let half_w = width * 0.5;
    let half_h = height * 0.5;
    let qx = cx.abs() - (half_w - radius);
    let qy = cy.abs() - (half_h - radius);
    let outside_x = qx.max(0.0);
    let outside_y = qy.max(0.0);
    let outside = (outside_x * outside_x + outside_y * outside_y).sqrt();
    let inside = qx.max(qy).min(0.0);
    let dist = outside + inside - radius;
    dist <= 0.75
}
