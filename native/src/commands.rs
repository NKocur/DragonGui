use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::{PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyAny;
use serde_json::Value;
use winit::event_loop::EventLoopProxy;

use crate::css_style::{parse_stylesheet, StylesheetOrigin};
use crate::document::{LinePlotPayloadFormat, ScatterPayloadFormat};
use crate::theme::Theme;

/// User event sent into the winit loop when the Python/Rust runtime bridge has
/// work waiting or a small event-loop-owned window operation is requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEvent {
    Wake,
    ResizeLogical { width: u32, height: u32 },
}

/// Coarse invalidation classes used by future command processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dirty {
    Layout,
    Text,
    Visual,
    GpuData,
    Full,
}

impl Dirty {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "layout" | "Layout" => Some(Self::Layout),
            "text" | "Text" => Some(Self::Text),
            "visual" | "Visual" => Some(Self::Visual),
            "gpu_data" | "GpuData" | "gpuData" => Some(Self::GpuData),
            "full" | "Full" => Some(Self::Full),
            _ => None,
        }
    }
}

/// Small value payload for early SetProp commands.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandValue {
    None,
    Bool(bool),
    Float(f32),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropUpdate {
    pub id: String,
    pub prop: String,
    pub value: CommandValue,
}

/// Native runtime command consumed by the winit UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetProp {
        id: String,
        prop: String,
        value: CommandValue,
    },
    SetProps {
        updates: Vec<PropUpdate>,
    },
    SetStyle {
        id: String,
        patch_json: String,
    },
    ReplaceChildren {
        id: String,
        children_json: String,
    },
    ReplaceNode {
        id: String,
        node_json: String,
    },
    UpdateExtensionDisplayList {
        id: String,
        display_list_json: String,
    },
    PrewarmScatterWidgets {
        count: usize,
    },
    SetScatterPointsPacked {
        id: String,
        xyz: Vec<u8>,
        telemetry: Option<ScatterTelemetry>,
        colormap: String,
        /// Wire format; defaults to xyz_f32_v0 when absent.
        payload_format: ScatterPayloadFormat,
        /// Refit the camera to this payload's bounds after upload.
        fit: bool,
        /// When true, older pending point updates for this scatter are dropped.
        coalesce: bool,
    },
    SetLinePlotDataPacked {
        id: String,
        series: String,
        xy: Vec<u8>,
        label: Option<String>,
        color: Option<String>,
        line_width: Option<f32>,
        line_style: Option<String>,
        show_grid: Option<bool>,
        auto_fit: Option<bool>,
        max_points: Option<usize>,
        payload_format: LinePlotPayloadFormat,
        fit: bool,
        coalesce: bool,
    },
    SetHistogramData {
        id: String,
        edges: Vec<f32>,
        counts: Vec<f32>,
        input_count: usize,
        finite_count: usize,
        auto_fit: bool,
        coalesce: bool,
    },
    AppendLinePlotPointsPacked {
        id: String,
        series: String,
        xy: Vec<u8>,
        max_points: Option<usize>,
        payload_format: LinePlotPayloadFormat,
    },
    ClearLinePlotSeries {
        id: String,
        series: Option<String>,
    },
    SetScatterPrimaryHoverMeta {
        id: String,
        /// JSON array of per-point tooltip strings for the primary buffer.
        meta: String,
    },
    SetScatterPrimaryHoverColumns {
        id: String,
        columns: Vec<ScatterHoverColumnPacket>,
    },
    SetScatterTooltipAxisLabels {
        id: String,
        /// Column names used as coordinate labels in the hover tooltip (x, y, z order).
        labels: [String; 3],
    },
    SetTableData {
        id: String,
        table_json: String,
    },
    SetTableDataColumns {
        id: String,
        table_json: String,
        columns: Vec<TableColumnPacket>,
    },
    SetBufferResource {
        id: String,
        kind: String,
        bytes: Vec<u8>,
        owner_id: Option<String>,
    },
    ReleaseResource {
        id: String,
    },
    SetStylesheet {
        origin: StylesheetOrigin,
        id: Option<String>,
        css: String,
    },
    RemoveStylesheet {
        origin: StylesheetOrigin,
        id: String,
    },
    SetTheme {
        theme: Theme,
    },
    SetIconTheme {
        theme: Value,
    },
    ClearStylesheets {
        origin: StylesheetOrigin,
    },
    ShowToast {
        id: String,
        message: String,
        level: String,
        duration_ms: Option<u64>,
        opacity: Option<f32>,
        radius: Option<f32>,
        padding: Option<f32>,
        position: Option<String>,
    },
    DismissToast {
        id: String,
    },
    Invalidate {
        id: String,
        dirty: Dirty,
    },
    ResetScatterCamera {
        id: String,
    },
    SetScatterViewDirection {
        id: String,
        /// One of "xy", "xz", "yz", "isometric".
        direction: String,
    },
    SetScatterPointStyle {
        id: String,
        /// One of "circle", "square", "gaussian".
        style: String,
    },
    SetScatterPointSize {
        id: String,
        /// Logical pixels. Native scales this to the current monitor scale factor.
        size: f32,
    },
    FitScatterCamera {
        id: String,
        /// Optional explicit bounds: [x_min, y_min, z_min, x_max, y_max, z_max].
        /// When None the scatter refits to its current data bounds.
        bounds: Option<[f32; 6]>,
    },
    SetScatterParallelProjection {
        id: String,
        parallel: bool,
    },
    SetScatterCameraState {
        id: String,
        target: [f32; 3],
        distance: f32,
        yaw: f32,
        pitch: f32,
        parallel: bool,
    },
    SetScatterGridVisible {
        id: String,
        visible: bool,
    },
    SetScatterGridPlanes {
        id: String,
        major: bool,
        minor: bool,
    },
    SetScatterGridOptions {
        id: String,
        sticky: bool,
        all_edges: bool,
    },
    SetScatterTicks {
        id: String,
        x: Option<usize>,
        y: Option<usize>,
        z: Option<usize>,
    },
    SetScatterAxes {
        id: String,
        x: String,
        y: String,
        z: String,
    },
    SetScatterAxisVisibility {
        id: String,
        x: bool,
        y: bool,
        z: bool,
    },
    SetScatterBackground {
        id: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetScatterLegend {
        id: String,
        visible: bool,
        position: String,
        /// Flat list of (label, r, g, b) entries.
        entries: Vec<(String, f32, f32, f32)>,
        title: Option<String>,
    },
    SetScatterScalarBar {
        id: String,
        visible: bool,
        vmin: f32,
        vmax: f32,
        log_scale: bool,
        colormap: String,
        title: Option<String>,
    },
    SetScatterOrientationAxes {
        id: String,
        visible: bool,
    },
    AddScatterLabel {
        id: String,
        label_id: u32,
        x: f32,
        y: f32,
        z: f32,
        text: String,
        r: f32,
        g: f32,
        b: f32,
        size: f32,
        anchor: String,
    },
    UpdateScatterLabel {
        id: String,
        label_id: u32,
        x: Option<f32>,
        y: Option<f32>,
        z: Option<f32>,
        text: Option<String>,
        r: Option<f32>,
        g: Option<f32>,
        b: Option<f32>,
        size: Option<f32>,
        anchor: Option<String>,
    },
    RemoveScatterLabel {
        id: String,
        label_id: u32,
    },
    SetScatterLabelVisible {
        id: String,
        label_id: u32,
        visible: bool,
    },
    ClearScatterLabels {
        id: String,
    },
    AddScatterLines {
        id: String,
        overlay_id: u32,
        segments: Vec<[f32; 6]>,
        r: f32,
        g: f32,
        b: f32,
    },
    UpdateScatterLines {
        id: String,
        overlay_id: u32,
        segments: Vec<[f32; 6]>,
        r: f32,
        g: f32,
        b: f32,
    },
    AddScatterBox {
        id: String,
        overlay_id: u32,
        xmin: f32,
        xmax: f32,
        ymin: f32,
        ymax: f32,
        zmin: f32,
        zmax: f32,
        r: f32,
        g: f32,
        b: f32,
    },
    RemoveScatterOverlay {
        id: String,
        overlay_id: u32,
    },
    SetScatterOverlayVisible {
        id: String,
        overlay_id: u32,
        visible: bool,
    },
    ClearScatterOverlays {
        id: String,
    },
    AddScatterActor {
        id: String,
        actor_id: u32,
        payload_b64: String,
        colormap: String,
        payload_format: ScatterPayloadFormat,
        /// JSON array of per-point tooltip strings; None or empty = coordinate fallback.
        hover_meta: Option<String>,
        /// Source column names for coordinate rows in the hover tooltip.
        tooltip_axis_labels: [String; 3],
    },
    AddScatterActorPacked {
        id: String,
        actor_id: u32,
        payload: Vec<u8>,
        colormap: String,
        payload_format: ScatterPayloadFormat,
        /// JSON array of per-point tooltip strings; None or empty = coordinate fallback.
        hover_meta: Option<String>,
        /// Source column names for coordinate rows in the hover tooltip.
        tooltip_axis_labels: [String; 3],
    },
    UpdateScatterActor {
        id: String,
        actor_id: u32,
        payload_b64: String,
        colormap: String,
        payload_format: ScatterPayloadFormat,
        /// Source column names for coordinate rows in the hover tooltip.
        tooltip_axis_labels: [String; 3],
    },
    UpdateScatterActorPacked {
        id: String,
        actor_id: u32,
        payload: Vec<u8>,
        colormap: String,
        payload_format: ScatterPayloadFormat,
        /// Source column names for coordinate rows in the hover tooltip.
        tooltip_axis_labels: [String; 3],
    },
    RemoveScatterActor {
        id: String,
        actor_id: u32,
    },
    SetScatterActorVisible {
        id: String,
        actor_id: u32,
        visible: bool,
    },
    ClearScatterActors {
        id: String,
    },
    ClearScatterScene {
        id: String,
    },
    AddScatterStream {
        id: String,
        actor_id: u32,
        max_points: u32,
        mode: String,
    },
    StreamScatterActor {
        id: String,
        actor_id: u32,
        payload_b64: String,
        colormap: String,
        payload_format: ScatterPayloadFormat,
    },
    StreamScatterActorPacked {
        id: String,
        actor_id: u32,
        payload: Vec<u8>,
        colormap: String,
        payload_format: ScatterPayloadFormat,
    },
    ClearScatterStream {
        id: String,
        actor_id: u32,
    },
    // ── Phase 5: Selection, Hover, LOD ───────────────────────────────────────
    SetScatterLod {
        id: String,
        enabled: bool,
        threshold: u32,
        factor: u32,
    },
    SetScatterAutoPointSize {
        id: String,
        enabled: bool,
    },
    SetScatterInteractiveRenderScale {
        id: String,
        scale: f32,
    },
    SetScatterAutoQuality {
        id: String,
        enabled: bool,
        target_fps: f32,
    },
    SetScatterPickingMode {
        id: String,
        /// "point" | "rectangle" | "lasso" | "none"
        mode: String,
    },
    SetScatterHoverTooltip {
        id: String,
        enabled: bool,
    },
    // ── Phase 6: Mesh and Statistical Overlays ────────────────────────────────
    AddScatterMesh {
        id: String,
        mesh_id: u32,
        /// Base64-encoded little-endian float32 (N × 3) vertex positions.
        positions_b64: String,
        /// Base64-encoded little-endian uint32 (M × 3) triangle indices.
        indices_b64: String,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        wireframe: bool,
    },
    UpdateScatterMesh {
        id: String,
        mesh_id: u32,
        positions_b64: Option<String>,
        indices_b64: Option<String>,
        r: Option<f32>,
        g: Option<f32>,
        b: Option<f32>,
        a: Option<f32>,
        wireframe: Option<bool>,
    },
    RemoveScatterMesh {
        id: String,
        mesh_id: u32,
    },
    SetScatterMeshVisible {
        id: String,
        mesh_id: u32,
        visible: bool,
    },
    ClearScatterMeshes {
        id: String,
    },
    // ── Phase 7: Export / Camera ──────────────────────────────────────────────
    SetScatterParallelScale {
        id: String,
        half_w: f32,
        half_h: f32,
    },
    ScatterScreenshot {
        id: String,
        request_id: u64,
    },
    WindowScreenshot {
        request_id: u64,
    },
    DebugSnapshot {
        request_id: u64,
    },
    LatencyProbe {
        request_id: u64,
    },
    DrainPythonTasks,
    RequestRedraw,
    RequestExit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScatterTelemetry {
    pub pack_ms: f64,
    pub enqueue_epoch_ms: f64,
    pub point_count: usize,
    pub payload_bytes: usize,
    pub bounds: Option<([f32; 3], [f32; 3])>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnPacket {
    pub name: String,
    pub dtype: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScatterHoverColumnPacket {
    pub name: String,
    pub dtype: String,
    pub len: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandQueueError {
    Closed,
}

#[derive(Debug)]
struct CommandQueueNode {
    command: Command,
    previous: Option<usize>,
    next: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum CommandCoalescingKey {
    SetProp(String, String),
    Theme,
    Stylesheet(StylesheetOrigin, Option<String>),
    ClearStylesheets(StylesheetOrigin),
    ScatterPoints(String),
    LinePlot(String, String),
    Histogram(String),
    ScatterScalarBar(String),
    ScatterActor(String, u32),
    ExtensionDisplayList(String),
    IconTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandCoalescingBarrier {
    Structural,
    Observation,
    Callback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum CommandCoalescingFamily {
    Property,
    PropertyPacket,
    Theme,
    Stylesheet,
    ScatterPoints,
    LinePlot,
    Histogram,
    ScatterScalarBar,
    ScatterActor,
    ExtensionDisplayList,
    IconTheme,
    LinePlotAppend,
}

impl CommandCoalescingFamily {
    pub(crate) const ALL: [Self; 12] = [
        Self::Property,
        Self::PropertyPacket,
        Self::Theme,
        Self::Stylesheet,
        Self::ScatterPoints,
        Self::LinePlot,
        Self::Histogram,
        Self::ScatterScalarBar,
        Self::ScatterActor,
        Self::ExtensionDisplayList,
        Self::IconTheme,
        Self::LinePlotAppend,
    ];
    pub(crate) const COUNT: usize = Self::ALL.len();

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Property => "property",
            Self::PropertyPacket => "property_packet",
            Self::Theme => "theme",
            Self::Stylesheet => "stylesheet",
            Self::ScatterPoints => "scatter_points",
            Self::LinePlot => "line_plot",
            Self::Histogram => "histogram",
            Self::ScatterScalarBar => "scatter_scalar_bar",
            Self::ScatterActor => "scatter_actor",
            Self::ExtensionDisplayList => "extension_display_list",
            Self::IconTheme => "icon_theme",
            Self::LinePlotAppend => "line_plot_append",
        }
    }
}

impl CommandCoalescingKey {
    fn property(id: &str, prop: &str) -> Self {
        Self::SetProp(id.to_string(), prop.to_string())
    }

    fn stylesheet_origin(&self) -> Option<StylesheetOrigin> {
        match self {
            Self::Stylesheet(origin, _) | Self::ClearStylesheets(origin) => Some(*origin),
            _ => None,
        }
    }
}

impl PropUpdate {
    pub(crate) fn coalescing_key(&self) -> CommandCoalescingKey {
        CommandCoalescingKey::property(&self.id, &self.prop)
    }
}

#[derive(Debug, Default)]
struct CommandQueueInner {
    nodes: Vec<Option<CommandQueueNode>>,
    free: Vec<usize>,
    latest: HashMap<CommandCoalescingKey, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    len: usize,
    stats: CommandQueueStats,
}

impl CommandQueueInner {
    fn unlink(&mut self, index: usize) -> Command {
        let node = self.nodes[index]
            .take()
            .expect("command queue index must reference a live node");
        if let Some(previous) = node.previous {
            self.nodes[previous]
                .as_mut()
                .expect("previous command queue node must be live")
                .next = node.next;
        } else {
            self.head = node.next;
        }
        if let Some(next) = node.next {
            self.nodes[next]
                .as_mut()
                .expect("next command queue node must be live")
                .previous = node.previous;
        } else {
            self.tail = node.previous;
        }
        self.len -= 1;
        self.free.push(index);
        node.command
    }

    fn remove_key(&mut self, key: &CommandCoalescingKey) -> Option<Command> {
        let index = self.latest.remove(key)?;
        Some(self.unlink(index))
    }

    fn append(&mut self, command: Command, key: Option<CommandCoalescingKey>) {
        let index = self.free.pop().unwrap_or(self.nodes.len());
        let node = CommandQueueNode {
            command,
            previous: self.tail,
            next: None,
        };
        if index == self.nodes.len() {
            self.nodes.push(Some(node));
        } else {
            self.nodes[index] = Some(node);
        }
        if let Some(tail) = self.tail {
            self.nodes[tail]
                .as_mut()
                .expect("tail command queue node must be live")
                .next = Some(index);
        } else {
            self.head = Some(index);
        }
        self.tail = Some(index);
        self.len += 1;
        if let Some(key) = key {
            self.latest.insert(key, index);
        }
    }

    fn pop_front(&mut self) -> Option<Command> {
        let index = self.head?;
        let key = self.nodes[index].as_ref()?.command.coalescing_key();
        if let Some(key) = key {
            if self.latest.get(&key) == Some(&index) {
                self.latest.remove(&key);
            }
        }
        Some(self.unlink(index))
    }
}

const COMMAND_QUEUE_TIMING_SAMPLE_WINDOW: usize = 128;

#[derive(Debug, Clone, Copy)]
enum QueueReplacementFamily {
    SetProp,
    ThemeStylesheet,
    ScatterPoints,
    LinePlot,
    Histogram,
    ScatterScalarBar,
    ScatterActor,
    ExtensionDisplayList,
    IconTheme,
    Other,
}

#[derive(Debug, Default)]
struct CommandQueueStats {
    pushes: u64,
    replacements: u64,
    set_prop_replacements: u64,
    theme_stylesheet_replacements: u64,
    scatter_point_replacements: u64,
    line_plot_replacements: u64,
    histogram_replacements: u64,
    scatter_scalar_bar_replacements: u64,
    scatter_actor_replacements: u64,
    extension_display_list_replacements: u64,
    icon_theme_replacements: u64,
    barrier_segments: u64,
    structural_barriers: u64,
    observation_barriers: u64,
    callback_barriers: u64,
    high_water: usize,
    push_ms: VecDeque<f64>,
    push_total_ms: f64,
    push_max_ms: f64,
}

impl CommandQueueStats {
    fn record_barrier(&mut self, barrier: Option<CommandCoalescingBarrier>) {
        let Some(barrier) = barrier else {
            return;
        };
        self.barrier_segments = self.barrier_segments.saturating_add(1);
        let class = match barrier {
            CommandCoalescingBarrier::Structural => &mut self.structural_barriers,
            CommandCoalescingBarrier::Observation => &mut self.observation_barriers,
            CommandCoalescingBarrier::Callback => &mut self.callback_barriers,
        };
        *class = class.saturating_add(1);
    }

    fn record_push(
        &mut self,
        elapsed_ms: f64,
        depth: usize,
        replacements: usize,
        family: QueueReplacementFamily,
    ) {
        self.pushes = self.pushes.saturating_add(1);
        self.replacements = self.replacements.saturating_add(replacements as u64);
        self.high_water = self.high_water.max(depth);
        self.push_total_ms += elapsed_ms;
        self.push_max_ms = self.push_max_ms.max(elapsed_ms);
        if self.push_ms.len() == COMMAND_QUEUE_TIMING_SAMPLE_WINDOW {
            self.push_ms.pop_front();
        }
        self.push_ms.push_back(elapsed_ms);
        let target = match family {
            QueueReplacementFamily::SetProp => &mut self.set_prop_replacements,
            QueueReplacementFamily::ThemeStylesheet => &mut self.theme_stylesheet_replacements,
            QueueReplacementFamily::ScatterPoints => &mut self.scatter_point_replacements,
            QueueReplacementFamily::LinePlot => &mut self.line_plot_replacements,
            QueueReplacementFamily::Histogram => &mut self.histogram_replacements,
            QueueReplacementFamily::ScatterScalarBar => &mut self.scatter_scalar_bar_replacements,
            QueueReplacementFamily::ScatterActor => &mut self.scatter_actor_replacements,
            QueueReplacementFamily::ExtensionDisplayList => {
                &mut self.extension_display_list_replacements
            }
            QueueReplacementFamily::IconTheme => &mut self.icon_theme_replacements,
            QueueReplacementFamily::Other => return,
        };
        *target = target.saturating_add(replacements as u64);
    }

    fn percentile_ms(&self, percentile: f64) -> f64 {
        if self.push_ms.is_empty() {
            return 0.0;
        }
        let mut samples = self.push_ms.iter().copied().collect::<Vec<_>>();
        samples.sort_by(f64::total_cmp);
        let rank = ((samples.len() - 1) as f64 * percentile.clamp(0.0, 1.0)).round() as usize;
        samples[rank]
    }

    fn json_value(&self, depth: usize, physical_slots: usize, free_slots: usize) -> Value {
        let avg_ms = if self.pushes == 0 {
            0.0
        } else {
            self.push_total_ms / self.pushes as f64
        };
        serde_json::json!({
            "depth": depth,
            "live_entries": depth,
            "physical_slots": physical_slots,
            "free_slots": free_slots,
            "stale_entries": 0,
            "compactions": 0,
            "peak_physical_queue_length": self.high_water,
            "high_water": self.high_water,
            "pushes": self.pushes,
            "replacements": self.replacements,
            "replacements_by_family": {
                "set_prop": self.set_prop_replacements,
                "theme_stylesheet": self.theme_stylesheet_replacements,
                "scatter_points": self.scatter_point_replacements,
                "line_plot": self.line_plot_replacements,
                "histogram": self.histogram_replacements,
                "scatter_scalar_bar": self.scatter_scalar_bar_replacements,
                "scatter_actor": self.scatter_actor_replacements,
                "extension_display_list": self.extension_display_list_replacements,
                "icon_theme": self.icon_theme_replacements,
            },
            "barrier_segments": {
                "total": self.barrier_segments,
                "by_class": {
                    "structural": self.structural_barriers,
                    "observation": self.observation_barriers,
                    "callback": self.callback_barriers,
                },
            },
            "push_timing": {
                "count": self.pushes,
                "avg_ms": avg_ms,
                "total_ms": self.push_total_ms,
                "max_ms": self.push_max_ms,
                "p50_ms": self.percentile_ms(0.50),
                "p95_ms": self.percentile_ms(0.95),
                "p99_ms": self.percentile_ms(0.99),
                "sample_window": self.push_ms.len(),
            },
        })
    }
}

impl Command {
    pub(crate) fn coalescing_key(&self) -> Option<CommandCoalescingKey> {
        match self {
            Command::SetProp { id, prop, .. } => Some(CommandCoalescingKey::property(id, prop)),
            Command::SetTheme { .. } => Some(CommandCoalescingKey::Theme),
            Command::SetStylesheet { origin, id, .. } => {
                Some(CommandCoalescingKey::Stylesheet(*origin, id.clone()))
            }
            Command::RemoveStylesheet { origin, id } => {
                Some(CommandCoalescingKey::Stylesheet(*origin, Some(id.clone())))
            }
            Command::ClearStylesheets { origin } => {
                Some(CommandCoalescingKey::ClearStylesheets(*origin))
            }
            Command::SetScatterPointsPacked {
                id, coalesce: true, ..
            } => Some(CommandCoalescingKey::ScatterPoints(id.clone())),
            Command::SetLinePlotDataPacked {
                id,
                series,
                coalesce: true,
                ..
            } => Some(CommandCoalescingKey::LinePlot(id.clone(), series.clone())),
            Command::SetHistogramData {
                id, coalesce: true, ..
            } => Some(CommandCoalescingKey::Histogram(id.clone())),
            Command::SetScatterScalarBar { id, .. } => {
                Some(CommandCoalescingKey::ScatterScalarBar(id.clone()))
            }
            Command::UpdateScatterActorPacked { id, actor_id, .. } => {
                Some(CommandCoalescingKey::ScatterActor(id.clone(), *actor_id))
            }
            Command::UpdateExtensionDisplayList { id, .. } => {
                Some(CommandCoalescingKey::ExtensionDisplayList(id.clone()))
            }
            Command::SetIconTheme { .. } => Some(CommandCoalescingKey::IconTheme),
            _ => None,
        }
    }

    pub(crate) fn merge_replaced(&mut self, previous: &Command) -> bool {
        match (self, previous) {
            (
                Command::SetHistogramData { auto_fit, .. },
                Command::SetHistogramData {
                    auto_fit: previous_auto_fit,
                    ..
                },
            ) => {
                *auto_fit |= *previous_auto_fit;
                true
            }
            (
                Command::SetScatterPointsPacked { fit, .. },
                Command::SetScatterPointsPacked {
                    fit: previous_fit, ..
                },
            )
            | (
                Command::SetLinePlotDataPacked { fit, .. },
                Command::SetLinePlotDataPacked {
                    fit: previous_fit, ..
                },
            ) => {
                *fit |= *previous_fit;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn coalescing_family(&self) -> Option<CommandCoalescingFamily> {
        match self {
            Command::SetProp { .. } => Some(CommandCoalescingFamily::Property),
            Command::SetProps { .. } => Some(CommandCoalescingFamily::PropertyPacket),
            Command::SetTheme { .. } => Some(CommandCoalescingFamily::Theme),
            Command::SetStylesheet { .. }
            | Command::RemoveStylesheet { .. }
            | Command::ClearStylesheets { .. } => Some(CommandCoalescingFamily::Stylesheet),
            Command::SetScatterPointsPacked { coalesce: true, .. } => {
                Some(CommandCoalescingFamily::ScatterPoints)
            }
            Command::SetLinePlotDataPacked { coalesce: true, .. } => {
                Some(CommandCoalescingFamily::LinePlot)
            }
            Command::SetHistogramData { coalesce: true, .. } => {
                Some(CommandCoalescingFamily::Histogram)
            }
            Command::SetScatterScalarBar { .. } => Some(CommandCoalescingFamily::ScatterScalarBar),
            Command::UpdateScatterActorPacked { .. } => Some(CommandCoalescingFamily::ScatterActor),
            Command::UpdateExtensionDisplayList { .. } => {
                Some(CommandCoalescingFamily::ExtensionDisplayList)
            }
            Command::SetIconTheme { .. } => Some(CommandCoalescingFamily::IconTheme),
            Command::AppendLinePlotPointsPacked { .. } => {
                Some(CommandCoalescingFamily::LinePlotAppend)
            }
            _ => None,
        }
    }

    pub(crate) fn coalescing_barrier(&self) -> Option<CommandCoalescingBarrier> {
        match self {
            Command::ReplaceChildren { .. } | Command::ReplaceNode { .. } => {
                Some(CommandCoalescingBarrier::Structural)
            }
            Command::ScatterScreenshot { .. }
            | Command::WindowScreenshot { .. }
            | Command::DebugSnapshot { .. }
            | Command::LatencyProbe { .. } => Some(CommandCoalescingBarrier::Observation),
            Command::DrainPythonTasks => Some(CommandCoalescingBarrier::Callback),
            _ => None,
        }
    }

    pub(crate) fn try_merge_adjacent(&mut self, next: Command) -> Result<(), Command> {
        match (self, next) {
            (
                Command::AppendLinePlotPointsPacked {
                    id,
                    series,
                    xy,
                    max_points,
                    payload_format,
                },
                Command::AppendLinePlotPointsPacked {
                    id: next_id,
                    series: next_series,
                    xy: next_xy,
                    max_points: next_max_points,
                    payload_format: next_payload_format,
                },
            ) if *id == next_id
                && *series == next_series
                && *max_points == next_max_points
                && *payload_format == next_payload_format =>
            {
                xy.extend(next_xy);
                Ok(())
            }
            (_, next) => Err(next),
        }
    }
}

/// Thread-safe command queue shared by Python-facing senders and the future UI
/// thread bridge.
#[derive(Debug, Default)]
pub struct CommandQueue {
    closed: AtomicBool,
    inner: Mutex<CommandQueueInner>,
}

impl CommandQueue {
    pub fn push(&self, mut command: Command) -> Result<(), CommandQueueError> {
        let push_t0 = Instant::now();
        if self.is_closed() {
            return Err(CommandQueueError::Closed);
        }
        let mut inner = self.inner.lock().expect("command queue mutex poisoned");
        let barrier = command.coalescing_barrier();
        let depth_before = inner.len;
        let replacement_family = match &command {
            Command::SetProp { .. } => QueueReplacementFamily::SetProp,
            Command::SetTheme { .. }
            | Command::SetStylesheet { .. }
            | Command::RemoveStylesheet { .. }
            | Command::ClearStylesheets { .. } => QueueReplacementFamily::ThemeStylesheet,
            Command::SetScatterPointsPacked { .. } => QueueReplacementFamily::ScatterPoints,
            Command::SetLinePlotDataPacked { .. } => QueueReplacementFamily::LinePlot,
            Command::SetHistogramData { .. } => QueueReplacementFamily::Histogram,
            Command::SetScatterScalarBar { .. } => QueueReplacementFamily::ScatterScalarBar,
            Command::UpdateScatterActorPacked { .. } => QueueReplacementFamily::ScatterActor,
            Command::UpdateExtensionDisplayList { .. } => {
                QueueReplacementFamily::ExtensionDisplayList
            }
            Command::SetIconTheme { .. } => QueueReplacementFamily::IconTheme,
            _ => QueueReplacementFamily::Other,
        };
        let mut replacements = 0;
        if barrier.is_some() {
            inner.latest.clear();
        } else if let Command::ClearStylesheets { origin } = &command {
            let keys = inner
                .latest
                .keys()
                .filter(|key| key.stylesheet_origin() == Some(*origin))
                .cloned()
                .collect::<Vec<_>>();
            for key in keys {
                if inner.remove_key(&key).is_some() {
                    replacements += 1;
                }
            }
        } else if let Some(key) = command.coalescing_key() {
            if let Some(previous) = inner.remove_key(&key) {
                command.merge_replaced(&previous);
                replacements = 1;
            }
        }
        debug_assert_eq!(replacements, depth_before.saturating_sub(inner.len));
        let key = command.coalescing_key();
        inner.append(command, key);
        let depth = inner.len;
        inner.stats.record_barrier(barrier);
        inner.stats.record_push(
            push_t0.elapsed().as_secs_f64() * 1000.0,
            depth,
            replacements,
            replacement_family,
        );
        Ok(())
    }

    pub fn drain(&self) -> Vec<Command> {
        let mut inner = self.inner.lock().expect("command queue mutex poisoned");
        let mut drained = Vec::with_capacity(inner.len);
        while let Some(command) = inner.pop_front() {
            drained.push(command);
        }
        drained
    }

    pub fn drain_into(&self, out: &mut Vec<Command>) {
        let mut inner = self.inner.lock().expect("command queue mutex poisoned");
        while let Some(command) = inner.pop_front() {
            out.push(command);
        }
    }

    pub fn drain_limited_into(&self, out: &mut Vec<Command>, limit: usize) {
        if limit == 0 {
            return;
        }
        let mut inner = self.inner.lock().expect("command queue mutex poisoned");
        for _ in 0..limit {
            let Some(command) = inner.pop_front() else {
                break;
            };
            out.push(command);
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("command queue mutex poisoned").len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn debug_snapshot(&self) -> Value {
        let inner = self.inner.lock().expect("command queue mutex poisoned");
        inner
            .stats
            .json_value(inner.len, inner.nodes.len(), inner.free.len())
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

/// Shared runtime bridge used by Python-facing senders and the winit UI thread.
#[derive(Debug, Default)]
pub struct CommandBridge {
    queue: Arc<CommandQueue>,
    proxy: Mutex<Option<EventLoopProxy<RuntimeEvent>>>,
    wake_pending: AtomicBool,
    response_seq: AtomicU64,
    pending_responses: Mutex<HashMap<u64, Option<String>>>,
    response_cv: Condvar,
}

impl CommandBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, command: Command) -> Result<(), CommandQueueError> {
        self.queue.push(command)?;
        self.wake();
        Ok(())
    }

    pub fn drain(&self) -> Vec<Command> {
        self.queue.drain()
    }

    pub fn drain_into(&self, out: &mut Vec<Command>) {
        self.queue.drain_into(out);
    }

    pub fn drain_limited_into(&self, out: &mut Vec<Command>, limit: usize) {
        self.queue.drain_limited_into(out, limit);
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn queue_debug_snapshot(&self) -> Value {
        self.queue.debug_snapshot()
    }

    pub fn close(&self) {
        self.queue.close();
        self.proxy
            .lock()
            .expect("command bridge proxy mutex poisoned")
            .take();
        self.response_cv.notify_all();
    }

    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    pub fn install_proxy(&self, proxy: EventLoopProxy<RuntimeEvent>) {
        *self
            .proxy
            .lock()
            .expect("command bridge proxy mutex poisoned") = Some(proxy);
        if !self.is_empty() {
            self.wake();
        }
    }

    pub fn wake(&self) {
        if self.is_closed() {
            return;
        }
        if self.wake_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let proxy = self
            .proxy
            .lock()
            .expect("command bridge proxy mutex poisoned")
            .clone();
        if let Some(proxy) = proxy {
            if proxy.send_event(RuntimeEvent::Wake).is_err() {
                self.wake_pending.store(false, Ordering::Release);
            }
        } else {
            self.wake_pending.store(false, Ordering::Release);
        }
    }

    pub fn request_window_resize(&self, width: u32, height: u32) -> bool {
        if self.is_closed() {
            return false;
        }
        let proxy = self
            .proxy
            .lock()
            .expect("command bridge proxy mutex poisoned")
            .clone();
        proxy.is_some_and(|proxy| {
            proxy
                .send_event(RuntimeEvent::ResizeLogical {
                    width: width.max(1),
                    height: height.max(1),
                })
                .is_ok()
        })
    }

    pub fn clear_wake_pending(&self) {
        self.wake_pending.store(false, Ordering::Release);
    }

    pub fn request_debug_snapshot(&self, timeout: Duration) -> Result<String, SnapshotError> {
        self.request_response(
            timeout,
            |request_id| Command::DebugSnapshot { request_id },
            "debug snapshot response disappeared",
        )
    }

    pub fn request_latency_probe(&self, timeout: Duration) -> Result<String, SnapshotError> {
        self.request_response(
            timeout,
            |request_id| Command::LatencyProbe { request_id },
            "latency probe response disappeared",
        )
    }

    fn request_response(
        &self,
        timeout: Duration,
        make_command: impl FnOnce(u64) -> Command,
        disappeared_message: &'static str,
    ) -> Result<String, SnapshotError> {
        if self.is_closed() {
            return Err(SnapshotError::Closed);
        }

        let request_id = self.response_seq.fetch_add(1, Ordering::Relaxed);
        {
            let mut responses = self
                .pending_responses
                .lock()
                .expect("command bridge response mutex poisoned");
            responses.insert(request_id, None);
        }

        if self.push(make_command(request_id)).is_err() {
            self.pending_responses
                .lock()
                .expect("command bridge response mutex poisoned")
                .remove(&request_id);
            return Err(SnapshotError::Closed);
        }

        let deadline = Instant::now() + timeout;
        let mut responses = self
            .pending_responses
            .lock()
            .expect("command bridge response mutex poisoned");
        loop {
            if responses
                .get(&request_id)
                .and_then(|slot| slot.as_ref())
                .is_some()
            {
                let response = responses
                    .remove(&request_id)
                    .and_then(|slot| slot)
                    .expect(disappeared_message);
                return Ok(response);
            }
            if self.is_closed() {
                responses.remove(&request_id);
                return Err(SnapshotError::Closed);
            }

            let now = Instant::now();
            if now >= deadline {
                responses.remove(&request_id);
                return Err(SnapshotError::Timeout);
            }
            let wait = deadline.saturating_duration_since(now);
            let (next, timeout_result) = self
                .response_cv
                .wait_timeout(responses, wait)
                .expect("command bridge response condvar poisoned");
            responses = next;
            if timeout_result.timed_out() {
                responses.remove(&request_id);
                return Err(SnapshotError::Timeout);
            }
        }
    }

    pub fn complete_response(&self, request_id: u64, response_json: String) {
        let mut responses = self
            .pending_responses
            .lock()
            .expect("command bridge response mutex poisoned");
        if let Some(slot) = responses.get_mut(&request_id) {
            *slot = Some(response_json);
            self.response_cv.notify_all();
        }
    }

    pub fn request_scatter_screenshot(
        &self,
        id: String,
        timeout: Duration,
    ) -> Result<String, SnapshotError> {
        self.request_response(
            timeout,
            |request_id| Command::ScatterScreenshot { id, request_id },
            "scatter screenshot response disappeared",
        )
    }

    pub fn request_window_screenshot(&self, timeout: Duration) -> Result<String, SnapshotError> {
        self.request_response(
            timeout,
            |request_id| Command::WindowScreenshot { request_id },
            "window screenshot response disappeared",
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    Closed,
    Timeout,
}

/// Private Python-facing sender scaffold.  It is intentionally underscored in
/// the module; public Python code should eventually reach it only through
/// AppHandle.
#[pyclass(name = "_NativeCommandSender")]
pub struct NativeCommandSender {
    bridge: Arc<CommandBridge>,
}

impl NativeCommandSender {
    pub fn new(bridge: Arc<CommandBridge>) -> Self {
        Self { bridge }
    }

    pub fn bridge(&self) -> Arc<CommandBridge> {
        Arc::clone(&self.bridge)
    }

    fn enqueue(&self, command: Command) -> PyResult<()> {
        self.bridge
            .push(command)
            .map_err(|_| PyRuntimeError::new_err("DragonGUI command sender is closed"))
    }
}

#[pymethods]
impl NativeCommandSender {
    #[new]
    fn py_new() -> Self {
        Self::new(Arc::new(CommandBridge::new()))
    }

    #[pyo3(name = "_queue_debug_snapshot")]
    fn py_queue_debug_snapshot(&self) -> PyResult<String> {
        serde_json::to_string(&self.bridge.queue_debug_snapshot())
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }

    #[pyo3(name = "_drain_for_test")]
    fn py_drain_for_test(&self) -> usize {
        self.bridge.drain().len()
    }

    fn enqueue_set_prop(&self, id: String, prop: String, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.enqueue(Command::SetProp {
            id,
            prop,
            value: command_value_from_py(value)?,
        })
    }

    fn enqueue_set_props(&self, updates: &Bound<'_, PyAny>) -> PyResult<()> {
        let raw_updates = updates.extract::<Vec<(String, String, Py<PyAny>)>>()?;
        if raw_updates.is_empty() {
            return Ok(());
        }
        let py = updates.py();
        let updates = raw_updates
            .into_iter()
            .map(|(id, prop, value)| {
                Ok(PropUpdate {
                    id,
                    prop,
                    value: command_value_from_py(value.bind(py))?,
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        self.enqueue(Command::SetProps { updates })
    }

    fn enqueue_set_style(&self, id: String, patch_json: String) -> PyResult<()> {
        let parsed: serde_json::Value = serde_json::from_str(&patch_json)
            .map_err(|e| PyValueError::new_err(format!("invalid style patch JSON: {e}")))?;
        if !parsed.is_object() {
            return Err(PyTypeError::new_err(
                "style patch must serialize to a JSON object",
            ));
        }
        self.enqueue(Command::SetStyle { id, patch_json })
    }

    fn enqueue_replace_children(&self, id: String, children_json: String) -> PyResult<()> {
        let parsed: serde_json::Value = serde_json::from_str(&children_json)
            .map_err(|e| PyValueError::new_err(format!("invalid children JSON: {e}")))?;
        if !parsed.is_array() {
            return Err(PyTypeError::new_err(
                "replacement children must serialize to a JSON array",
            ));
        }
        self.enqueue(Command::ReplaceChildren { id, children_json })
    }

    fn enqueue_replace_node(&self, id: String, node_json: String) -> PyResult<()> {
        let parsed: serde_json::Value = serde_json::from_str(&node_json)
            .map_err(|e| PyValueError::new_err(format!("invalid node JSON: {e}")))?;
        if !parsed.is_object() {
            return Err(PyTypeError::new_err(
                "replacement node must serialize to a JSON object",
            ));
        }
        self.enqueue(Command::ReplaceNode { id, node_json })
    }

    fn enqueue_update_extension_display_list(
        &self,
        id: String,
        display_list_json: String,
    ) -> PyResult<()> {
        let parsed: serde_json::Value = serde_json::from_str(&display_list_json)
            .map_err(|e| PyValueError::new_err(format!("invalid display-list JSON: {e}")))?;
        if !parsed.is_array() {
            return Err(PyTypeError::new_err(
                "extension display list must serialize to a JSON array",
            ));
        }
        self.enqueue(Command::UpdateExtensionDisplayList {
            id,
            display_list_json,
        })
    }

    fn enqueue_prewarm_scatter_widgets(&self, count: usize) -> PyResult<()> {
        self.enqueue(Command::PrewarmScatterWidgets {
            count: count.min(64),
        })
    }

    fn enqueue_invalidate(&self, id: String, dirty: String) -> PyResult<()> {
        let dirty = Dirty::from_str(&dirty)
            .ok_or_else(|| PyValueError::new_err(format!("unknown dirty flag: {dirty}")))?;
        self.enqueue(Command::Invalidate { id, dirty })
    }

    #[pyo3(signature = (id, xyz, pack_ms=None, enqueue_epoch_ms=None, colormap=None, payload_format=None, coalesce=None, fit=false, bounds_min=None, bounds_max=None))]
    fn enqueue_set_scatter_points_packed(
        &self,
        id: String,
        xyz: &Bound<'_, PyAny>,
        pack_ms: Option<f64>,
        enqueue_epoch_ms: Option<f64>,
        colormap: Option<String>,
        payload_format: Option<String>,
        coalesce: Option<bool>,
        fit: bool,
        bounds_min: Option<(f32, f32, f32)>,
        bounds_max: Option<(f32, f32, f32)>,
    ) -> PyResult<()> {
        let xyz = byte_buffer_from_py(xyz, "scatter point payload")?;
        let fmt = ScatterPayloadFormat::from_str(payload_format.as_deref().unwrap_or("xyz_f32_v0"));
        let bytes_per_point = match fmt {
            ScatterPayloadFormat::PointInstanceV1 => 32,
            ScatterPayloadFormat::XyzF32V0 => 12,
        };
        let point_count = if bytes_per_point > 0 {
            xyz.len() / bytes_per_point
        } else {
            0
        };
        let payload_bytes = xyz.len();
        let bounds = match (bounds_min, bounds_max) {
            (Some(min), Some(max))
                if min.0.is_finite()
                    && min.1.is_finite()
                    && min.2.is_finite()
                    && max.0.is_finite()
                    && max.1.is_finite()
                    && max.2.is_finite() =>
            {
                Some(([min.0, min.1, min.2], [max.0, max.1, max.2]))
            }
            _ => None,
        };
        let telemetry = Some(ScatterTelemetry {
            pack_ms: pack_ms.unwrap_or(0.0).max(0.0),
            enqueue_epoch_ms: enqueue_epoch_ms.unwrap_or_else(now_epoch_ms),
            point_count,
            payload_bytes,
            bounds,
        });
        self.enqueue(Command::SetScatterPointsPacked {
            id,
            xyz,
            telemetry,
            colormap: normalize_colormap(colormap),
            payload_format: fmt,
            fit,
            coalesce: coalesce.unwrap_or(true),
        })
    }

    #[pyo3(signature = (id, series, xy, label=None, color=None, line_width=None, line_style=None, show_grid=None, auto_fit=None, max_points=None, fit=true, coalesce=true))]
    fn enqueue_set_line_plot_data_packed(
        &self,
        id: String,
        series: String,
        xy: &Bound<'_, PyAny>,
        label: Option<String>,
        color: Option<String>,
        line_width: Option<f32>,
        line_style: Option<String>,
        show_grid: Option<bool>,
        auto_fit: Option<bool>,
        max_points: Option<usize>,
        fit: bool,
        coalesce: bool,
    ) -> PyResult<()> {
        let xy = byte_buffer_from_py(xy, "line plot xy payload")?;
        if xy.len() % 8 != 0 {
            return Err(PyValueError::new_err(format!(
                "line plot xy payload length {} is not a multiple of 8",
                xy.len()
            )));
        }
        self.enqueue(Command::SetLinePlotDataPacked {
            id,
            series,
            xy,
            label,
            color,
            line_width,
            line_style,
            show_grid,
            auto_fit,
            max_points,
            payload_format: LinePlotPayloadFormat::XyF32V0,
            fit,
            coalesce,
        })
    }

    #[pyo3(signature = (id, series, xy, max_points=None))]
    fn enqueue_append_line_plot_points_packed(
        &self,
        id: String,
        series: String,
        xy: &Bound<'_, PyAny>,
        max_points: Option<usize>,
    ) -> PyResult<()> {
        let xy = byte_buffer_from_py(xy, "line plot append payload")?;
        if xy.len() % 8 != 0 {
            return Err(PyValueError::new_err(format!(
                "line plot append payload length {} is not a multiple of 8",
                xy.len()
            )));
        }
        self.enqueue(Command::AppendLinePlotPointsPacked {
            id,
            series,
            xy,
            max_points,
            payload_format: LinePlotPayloadFormat::XyF32V0,
        })
    }

    #[pyo3(signature = (id, series=None))]
    fn enqueue_clear_line_plot_series(&self, id: String, series: Option<String>) -> PyResult<()> {
        self.enqueue(Command::ClearLinePlotSeries { id, series })
    }

    #[pyo3(signature = (id, edges, counts, input_count, finite_count, auto_fit=true, coalesce=true))]
    fn enqueue_set_histogram_data(
        &self,
        id: String,
        edges: Vec<f32>,
        counts: Vec<f32>,
        input_count: usize,
        finite_count: usize,
        auto_fit: bool,
        coalesce: bool,
    ) -> PyResult<()> {
        if edges.len() != counts.len().saturating_add(1) {
            return Err(PyValueError::new_err(
                "histogram edges length must equal counts length + 1",
            ));
        }
        if edges.len() < 2 {
            return Err(PyValueError::new_err(
                "histogram edges must contain at least two values",
            ));
        }
        if edges.iter().any(|value| !value.is_finite())
            || counts
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(PyValueError::new_err(
                "histogram edges/counts must be finite and counts must be non-negative",
            ));
        }
        if !edges.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(PyValueError::new_err(
                "histogram edges must be strictly increasing",
            ));
        }
        self.enqueue(Command::SetHistogramData {
            id,
            edges,
            counts,
            input_count,
            finite_count,
            auto_fit,
            coalesce,
        })
    }

    fn enqueue_reset_scatter_camera(&self, id: String) -> PyResult<()> {
        self.enqueue(Command::ResetScatterCamera { id })
    }

    #[pyo3(signature = (id, direction))]
    fn enqueue_set_scatter_view_direction(&self, id: String, direction: String) -> PyResult<()> {
        let dir = direction.trim().to_lowercase();
        if !matches!(dir.as_str(), "xy" | "xz" | "yz" | "isometric") {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown scatter view direction {:?}; expected one of: xy, xz, yz, isometric",
                direction
            )));
        }
        self.enqueue(Command::SetScatterViewDirection { id, direction: dir })
    }

    #[pyo3(signature = (id, style))]
    fn enqueue_set_scatter_point_style(&self, id: String, style: String) -> PyResult<()> {
        let s = style.trim().to_lowercase();
        if !matches!(s.as_str(), "circle" | "square" | "gaussian") {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown scatter point style {:?}; expected one of: circle, square, gaussian",
                style
            )));
        }
        self.enqueue(Command::SetScatterPointStyle { id, style: s })
    }

    #[pyo3(signature = (id, size))]
    fn enqueue_set_scatter_point_size(&self, id: String, size: f32) -> PyResult<()> {
        self.enqueue(Command::SetScatterPointSize {
            id,
            size: size.max(0.0),
        })
    }

    #[pyo3(signature = (id, bounds=None))]
    fn enqueue_fit_scatter_camera(&self, id: String, bounds: Option<[f32; 6]>) -> PyResult<()> {
        self.enqueue(Command::FitScatterCamera { id, bounds })
    }

    #[pyo3(signature = (id, parallel))]
    fn enqueue_set_scatter_parallel_projection(&self, id: String, parallel: bool) -> PyResult<()> {
        self.enqueue(Command::SetScatterParallelProjection { id, parallel })
    }

    #[pyo3(signature = (id, target, distance, yaw, pitch, parallel=false))]
    fn enqueue_set_scatter_camera_state(
        &self,
        id: String,
        target: [f32; 3],
        distance: f32,
        yaw: f32,
        pitch: f32,
        parallel: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterCameraState {
            id,
            target,
            distance,
            yaw,
            pitch,
            parallel,
        })
    }

    #[pyo3(signature = (id, visible))]
    fn enqueue_set_scatter_grid_visible(&self, id: String, visible: bool) -> PyResult<()> {
        self.enqueue(Command::SetScatterGridVisible { id, visible })
    }

    #[pyo3(signature = (id, major, minor=false))]
    fn enqueue_set_scatter_grid_planes(
        &self,
        id: String,
        major: bool,
        minor: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterGridPlanes { id, major, minor })
    }

    #[pyo3(signature = (id, sticky=true, all_edges=false))]
    fn enqueue_set_scatter_grid_options(
        &self,
        id: String,
        sticky: bool,
        all_edges: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterGridOptions {
            id,
            sticky,
            all_edges,
        })
    }

    #[pyo3(signature = (id, x=None, y=None, z=None))]
    fn enqueue_set_scatter_ticks(
        &self,
        id: String,
        x: Option<usize>,
        y: Option<usize>,
        z: Option<usize>,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterTicks { id, x, y, z })
    }

    #[pyo3(signature = (id, x, y, z))]
    fn enqueue_set_scatter_axes(
        &self,
        id: String,
        x: String,
        y: String,
        z: String,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterAxes { id, x, y, z })
    }

    #[pyo3(signature = (id, x, y, z))]
    fn enqueue_set_scatter_axis_visibility(
        &self,
        id: String,
        x: bool,
        y: bool,
        z: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterAxisVisibility { id, x, y, z })
    }

    #[pyo3(signature = (id, r, g, b))]
    fn enqueue_set_scatter_background(&self, id: String, r: f32, g: f32, b: f32) -> PyResult<()> {
        self.enqueue(Command::SetScatterBackground { id, r, g, b })
    }

    #[pyo3(signature = (id, visible, position, entries, title=None))]
    fn enqueue_set_scatter_legend(
        &self,
        id: String,
        visible: bool,
        position: String,
        entries: Vec<(String, f32, f32, f32)>,
        title: Option<String>,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterLegend {
            id,
            visible,
            position,
            entries,
            title,
        })
    }

    #[pyo3(signature = (id, visible, vmin, vmax, log_scale, colormap, title))]
    fn enqueue_set_scatter_scalar_bar(
        &self,
        id: String,
        visible: bool,
        vmin: f32,
        vmax: f32,
        log_scale: bool,
        colormap: String,
        title: Option<String>,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterScalarBar {
            id,
            visible,
            vmin,
            vmax,
            log_scale,
            colormap,
            title,
        })
    }

    #[pyo3(signature = (id, visible))]
    fn enqueue_set_scatter_orientation_axes(&self, id: String, visible: bool) -> PyResult<()> {
        self.enqueue(Command::SetScatterOrientationAxes { id, visible })
    }

    #[pyo3(signature = (id, label_id, x, y, z, text, r, g, b, size, anchor))]
    fn enqueue_add_scatter_label(
        &self,
        id: String,
        label_id: u32,
        x: f32,
        y: f32,
        z: f32,
        text: String,
        r: f32,
        g: f32,
        b: f32,
        size: f32,
        anchor: String,
    ) -> PyResult<()> {
        self.enqueue(Command::AddScatterLabel {
            id,
            label_id,
            x,
            y,
            z,
            text,
            r,
            g,
            b,
            size,
            anchor,
        })
    }

    #[pyo3(signature = (id, label_id, x, y, z, text, r, g, b, size, anchor))]
    fn enqueue_update_scatter_label(
        &self,
        id: String,
        label_id: u32,
        x: Option<f32>,
        y: Option<f32>,
        z: Option<f32>,
        text: Option<String>,
        r: Option<f32>,
        g: Option<f32>,
        b: Option<f32>,
        size: Option<f32>,
        anchor: Option<String>,
    ) -> PyResult<()> {
        self.enqueue(Command::UpdateScatterLabel {
            id,
            label_id,
            x,
            y,
            z,
            text,
            r,
            g,
            b,
            size,
            anchor,
        })
    }

    #[pyo3(signature = (id, label_id))]
    fn enqueue_remove_scatter_label(&self, id: String, label_id: u32) -> PyResult<()> {
        self.enqueue(Command::RemoveScatterLabel { id, label_id })
    }

    #[pyo3(signature = (id, label_id, visible))]
    fn enqueue_set_scatter_label_visible(
        &self,
        id: String,
        label_id: u32,
        visible: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterLabelVisible {
            id,
            label_id,
            visible,
        })
    }

    #[pyo3(signature = (id,))]
    fn enqueue_clear_scatter_labels(&self, id: String) -> PyResult<()> {
        self.enqueue(Command::ClearScatterLabels { id })
    }

    #[pyo3(signature = (id, overlay_id, segments, r, g, b))]
    fn enqueue_add_scatter_lines(
        &self,
        id: String,
        overlay_id: u32,
        segments: Vec<[f32; 6]>,
        r: f32,
        g: f32,
        b: f32,
    ) -> PyResult<()> {
        self.enqueue(Command::AddScatterLines {
            id,
            overlay_id,
            segments,
            r,
            g,
            b,
        })
    }

    #[pyo3(signature = (id, overlay_id, segments, r, g, b))]
    fn enqueue_update_scatter_lines(
        &self,
        id: String,
        overlay_id: u32,
        segments: Vec<[f32; 6]>,
        r: f32,
        g: f32,
        b: f32,
    ) -> PyResult<()> {
        self.enqueue(Command::UpdateScatterLines {
            id,
            overlay_id,
            segments,
            r,
            g,
            b,
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (id, overlay_id, xmin, xmax, ymin, ymax, zmin, zmax, r, g, b))]
    fn enqueue_add_scatter_box(
        &self,
        id: String,
        overlay_id: u32,
        xmin: f32,
        xmax: f32,
        ymin: f32,
        ymax: f32,
        zmin: f32,
        zmax: f32,
        r: f32,
        g: f32,
        b: f32,
    ) -> PyResult<()> {
        self.enqueue(Command::AddScatterBox {
            id,
            overlay_id,
            xmin,
            xmax,
            ymin,
            ymax,
            zmin,
            zmax,
            r,
            g,
            b,
        })
    }

    #[pyo3(signature = (id, overlay_id))]
    fn enqueue_remove_scatter_overlay(&self, id: String, overlay_id: u32) -> PyResult<()> {
        self.enqueue(Command::RemoveScatterOverlay { id, overlay_id })
    }

    #[pyo3(signature = (id, overlay_id, visible))]
    fn enqueue_set_scatter_overlay_visible(
        &self,
        id: String,
        overlay_id: u32,
        visible: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterOverlayVisible {
            id,
            overlay_id,
            visible,
        })
    }

    #[pyo3(signature = (id,))]
    fn enqueue_clear_scatter_overlays(&self, id: String) -> PyResult<()> {
        self.enqueue(Command::ClearScatterOverlays { id })
    }

    #[pyo3(signature = (id, actor_id, payload_b64, colormap, payload_format, hover_meta=None, tooltip_x=None, tooltip_y=None, tooltip_z=None))]
    fn enqueue_add_scatter_actor(
        &self,
        id: String,
        actor_id: u32,
        payload_b64: String,
        colormap: String,
        payload_format: String,
        hover_meta: Option<String>,
        tooltip_x: Option<String>,
        tooltip_y: Option<String>,
        tooltip_z: Option<String>,
    ) -> PyResult<()> {
        self.enqueue(Command::AddScatterActor {
            id,
            actor_id,
            payload_b64,
            colormap,
            payload_format: ScatterPayloadFormat::from_str(&payload_format),
            hover_meta,
            tooltip_axis_labels: [
                tooltip_x.unwrap_or_else(|| "x".to_string()),
                tooltip_y.unwrap_or_else(|| "y".to_string()),
                tooltip_z.unwrap_or_else(|| "z".to_string()),
            ],
        })
    }

    #[pyo3(signature = (id, actor_id, payload, colormap, payload_format, hover_meta=None, tooltip_x=None, tooltip_y=None, tooltip_z=None))]
    fn enqueue_add_scatter_actor_packed(
        &self,
        id: String,
        actor_id: u32,
        payload: &Bound<'_, PyAny>,
        colormap: String,
        payload_format: String,
        hover_meta: Option<String>,
        tooltip_x: Option<String>,
        tooltip_y: Option<String>,
        tooltip_z: Option<String>,
    ) -> PyResult<()> {
        self.enqueue(Command::AddScatterActorPacked {
            id,
            actor_id,
            payload: byte_buffer_from_py(payload, "scatter actor payload")?,
            colormap,
            payload_format: ScatterPayloadFormat::from_str(&payload_format),
            hover_meta,
            tooltip_axis_labels: [
                tooltip_x.unwrap_or_else(|| "x".to_string()),
                tooltip_y.unwrap_or_else(|| "y".to_string()),
                tooltip_z.unwrap_or_else(|| "z".to_string()),
            ],
        })
    }

    #[pyo3(signature = (id, actor_id, payload_b64, colormap, payload_format, tooltip_x=None, tooltip_y=None, tooltip_z=None))]
    fn enqueue_update_scatter_actor(
        &self,
        id: String,
        actor_id: u32,
        payload_b64: String,
        colormap: String,
        payload_format: String,
        tooltip_x: Option<String>,
        tooltip_y: Option<String>,
        tooltip_z: Option<String>,
    ) -> PyResult<()> {
        self.enqueue(Command::UpdateScatterActor {
            id,
            actor_id,
            payload_b64,
            colormap,
            payload_format: ScatterPayloadFormat::from_str(&payload_format),
            tooltip_axis_labels: [
                tooltip_x.unwrap_or_else(|| "x".to_string()),
                tooltip_y.unwrap_or_else(|| "y".to_string()),
                tooltip_z.unwrap_or_else(|| "z".to_string()),
            ],
        })
    }

    #[pyo3(signature = (id, actor_id, payload, colormap, payload_format, tooltip_x=None, tooltip_y=None, tooltip_z=None))]
    fn enqueue_update_scatter_actor_packed(
        &self,
        id: String,
        actor_id: u32,
        payload: &Bound<'_, PyAny>,
        colormap: String,
        payload_format: String,
        tooltip_x: Option<String>,
        tooltip_y: Option<String>,
        tooltip_z: Option<String>,
    ) -> PyResult<()> {
        self.enqueue(Command::UpdateScatterActorPacked {
            id,
            actor_id,
            payload: byte_buffer_from_py(payload, "scatter actor payload")?,
            colormap,
            payload_format: ScatterPayloadFormat::from_str(&payload_format),
            tooltip_axis_labels: [
                tooltip_x.unwrap_or_else(|| "x".to_string()),
                tooltip_y.unwrap_or_else(|| "y".to_string()),
                tooltip_z.unwrap_or_else(|| "z".to_string()),
            ],
        })
    }

    #[pyo3(signature = (id, actor_id))]
    fn enqueue_remove_scatter_actor(&self, id: String, actor_id: u32) -> PyResult<()> {
        self.enqueue(Command::RemoveScatterActor { id, actor_id })
    }

    #[pyo3(signature = (id, actor_id, visible))]
    fn enqueue_set_scatter_actor_visible(
        &self,
        id: String,
        actor_id: u32,
        visible: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterActorVisible {
            id,
            actor_id,
            visible,
        })
    }

    #[pyo3(signature = (id,))]
    fn enqueue_clear_scatter_actors(&self, id: String) -> PyResult<()> {
        self.enqueue(Command::ClearScatterActors { id })
    }

    #[pyo3(signature = (id,))]
    fn enqueue_clear_scatter_scene(&self, id: String) -> PyResult<()> {
        self.enqueue(Command::ClearScatterScene { id })
    }

    #[pyo3(signature = (id, actor_id, max_points, mode))]
    fn enqueue_add_scatter_stream(
        &self,
        id: String,
        actor_id: u32,
        max_points: u32,
        mode: String,
    ) -> PyResult<()> {
        self.enqueue(Command::AddScatterStream {
            id,
            actor_id,
            max_points,
            mode,
        })
    }

    #[pyo3(signature = (id, actor_id, payload_b64, colormap, payload_format))]
    fn enqueue_stream_scatter_actor(
        &self,
        id: String,
        actor_id: u32,
        payload_b64: String,
        colormap: String,
        payload_format: String,
    ) -> PyResult<()> {
        self.enqueue(Command::StreamScatterActor {
            id,
            actor_id,
            payload_b64,
            colormap,
            payload_format: ScatterPayloadFormat::from_str(&payload_format),
        })
    }

    #[pyo3(signature = (id, actor_id, payload, colormap, payload_format))]
    fn enqueue_stream_scatter_actor_packed(
        &self,
        id: String,
        actor_id: u32,
        payload: &Bound<'_, PyAny>,
        colormap: String,
        payload_format: String,
    ) -> PyResult<()> {
        self.enqueue(Command::StreamScatterActorPacked {
            id,
            actor_id,
            payload: byte_buffer_from_py(payload, "scatter stream payload")?,
            colormap,
            payload_format: ScatterPayloadFormat::from_str(&payload_format),
        })
    }

    #[pyo3(signature = (id, actor_id))]
    fn enqueue_clear_scatter_stream(&self, id: String, actor_id: u32) -> PyResult<()> {
        self.enqueue(Command::ClearScatterStream { id, actor_id })
    }

    #[pyo3(signature = (id, enabled, threshold, factor))]
    fn enqueue_set_scatter_lod(
        &self,
        id: String,
        enabled: bool,
        threshold: u32,
        factor: u32,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterLod {
            id,
            enabled,
            threshold,
            factor,
        })
    }

    #[pyo3(signature = (id, enabled))]
    fn enqueue_set_scatter_auto_point_size(&self, id: String, enabled: bool) -> PyResult<()> {
        self.enqueue(Command::SetScatterAutoPointSize { id, enabled })
    }

    #[pyo3(signature = (id, scale))]
    fn enqueue_set_scatter_interactive_render_scale(&self, id: String, scale: f32) -> PyResult<()> {
        self.enqueue(Command::SetScatterInteractiveRenderScale { id, scale })
    }

    #[pyo3(signature = (id, enabled, target_fps))]
    fn enqueue_set_scatter_auto_quality(
        &self,
        id: String,
        enabled: bool,
        target_fps: f32,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterAutoQuality {
            id,
            enabled,
            target_fps,
        })
    }

    #[pyo3(signature = (id, mode))]
    fn enqueue_set_scatter_picking_mode(&self, id: String, mode: String) -> PyResult<()> {
        self.enqueue(Command::SetScatterPickingMode { id, mode })
    }

    #[pyo3(signature = (id, enabled))]
    fn enqueue_set_scatter_hover_tooltip(&self, id: String, enabled: bool) -> PyResult<()> {
        self.enqueue(Command::SetScatterHoverTooltip { id, enabled })
    }

    #[pyo3(signature = (id, meta))]
    fn enqueue_set_scatter_primary_hover_meta(&self, id: String, meta: String) -> PyResult<()> {
        self.enqueue(Command::SetScatterPrimaryHoverMeta { id, meta })
    }

    fn enqueue_set_scatter_primary_hover_columns(
        &self,
        id: String,
        columns_json: String,
        buffers: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let metadata: serde_json::Value = serde_json::from_str(&columns_json).map_err(|e| {
            PyValueError::new_err(format!("invalid scatter hover columns JSON: {e}"))
        })?;
        let metadata = metadata.as_array().ok_or_else(|| {
            PyTypeError::new_err("scatter hover column metadata must serialize to a JSON array")
        })?;
        let buffers = byte_buffers_from_py_iterable(buffers, "scatter hover column buffer")?;
        if metadata.len() != buffers.len() {
            return Err(PyValueError::new_err(
                "scatter hover column metadata and buffer counts must match",
            ));
        }
        let mut columns = Vec::with_capacity(metadata.len());
        for (meta, bytes) in metadata.iter().zip(buffers) {
            let name = meta.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                PyValueError::new_err("scatter hover column metadata requires name")
            })?;
            let dtype = meta.get("dtype").and_then(|v| v.as_str()).ok_or_else(|| {
                PyValueError::new_err("scatter hover column metadata requires dtype")
            })?;
            let len = meta.get("len").and_then(|v| v.as_u64()).ok_or_else(|| {
                PyValueError::new_err("scatter hover column metadata requires len")
            })? as usize;
            columns.push(ScatterHoverColumnPacket {
                name: name.to_string(),
                dtype: dtype.to_string(),
                len,
                bytes,
            });
        }
        self.enqueue(Command::SetScatterPrimaryHoverColumns { id, columns })
    }

    #[pyo3(signature = (id, x, y, z))]
    fn enqueue_set_scatter_tooltip_axis_labels(
        &self,
        id: String,
        x: String,
        y: String,
        z: String,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterTooltipAxisLabels {
            id,
            labels: [x, y, z],
        })
    }

    #[pyo3(signature = (id, mesh_id, positions_b64, indices_b64, r, g, b, a, wireframe))]
    fn enqueue_add_scatter_mesh(
        &self,
        id: String,
        mesh_id: u32,
        positions_b64: String,
        indices_b64: String,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        wireframe: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::AddScatterMesh {
            id,
            mesh_id,
            positions_b64,
            indices_b64,
            r,
            g,
            b,
            a,
            wireframe,
        })
    }

    #[pyo3(signature = (id, mesh_id, positions_b64=None, indices_b64=None, r=None, g=None, b=None, a=None, wireframe=None))]
    fn enqueue_update_scatter_mesh(
        &self,
        id: String,
        mesh_id: u32,
        positions_b64: Option<String>,
        indices_b64: Option<String>,
        r: Option<f32>,
        g: Option<f32>,
        b: Option<f32>,
        a: Option<f32>,
        wireframe: Option<bool>,
    ) -> PyResult<()> {
        self.enqueue(Command::UpdateScatterMesh {
            id,
            mesh_id,
            positions_b64,
            indices_b64,
            r,
            g,
            b,
            a,
            wireframe,
        })
    }

    #[pyo3(signature = (id, mesh_id))]
    fn enqueue_remove_scatter_mesh(&self, id: String, mesh_id: u32) -> PyResult<()> {
        self.enqueue(Command::RemoveScatterMesh { id, mesh_id })
    }

    #[pyo3(signature = (id, mesh_id, visible))]
    fn enqueue_set_scatter_mesh_visible(
        &self,
        id: String,
        mesh_id: u32,
        visible: bool,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterMeshVisible {
            id,
            mesh_id,
            visible,
        })
    }

    #[pyo3(signature = (id))]
    fn enqueue_clear_scatter_meshes(&self, id: String) -> PyResult<()> {
        self.enqueue(Command::ClearScatterMeshes { id })
    }

    #[pyo3(signature = (id, half_w, half_h))]
    fn enqueue_set_scatter_parallel_scale(
        &self,
        id: String,
        half_w: f32,
        half_h: f32,
    ) -> PyResult<()> {
        self.enqueue(Command::SetScatterParallelScale { id, half_w, half_h })
    }

    #[pyo3(signature = (id, timeout_ms=10000))]
    fn scatter_screenshot(
        &self,
        id: String,
        timeout_ms: u64,
    ) -> PyResult<Option<(u32, u32, Vec<u8>)>> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let timeout = Duration::from_millis(timeout_ms);
        let json_str = self
            .bridge
            .request_scatter_screenshot(id, timeout)
            .map_err(|e| PyRuntimeError::new_err(format!("screenshot failed: {e:?}")))?;
        let v: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| PyRuntimeError::new_err(format!("screenshot JSON invalid: {e}")))?;
        let w = v["w"].as_u64().unwrap_or(0) as u32;
        let h = v["h"].as_u64().unwrap_or(0) as u32;
        let b64 = v["rgba_b64"].as_str().unwrap_or("");
        let bytes = STANDARD
            .decode(b64)
            .map_err(|e| PyRuntimeError::new_err(format!("screenshot base64 decode: {e}")))?;
        Ok(Some((w, h, bytes)))
    }

    #[pyo3(signature = (timeout_ms=10000))]
    fn window_screenshot(
        &self,
        py: Python<'_>,
        timeout_ms: u64,
    ) -> PyResult<Option<(u32, u32, Vec<u8>)>> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let timeout = Duration::from_millis(timeout_ms);
        let json_str = py
            .detach(|| self.bridge.request_window_screenshot(timeout))
            .map_err(|e| PyRuntimeError::new_err(format!("window screenshot failed: {e:?}")))?;
        let v: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| PyRuntimeError::new_err(format!("window screenshot JSON invalid: {e}")))?;
        if let Some(error) = v["error"].as_str().filter(|error| !error.is_empty()) {
            return Err(PyRuntimeError::new_err(format!(
                "window screenshot failed: {error}"
            )));
        }
        let w = v["w"].as_u64().unwrap_or(0) as u32;
        let h = v["h"].as_u64().unwrap_or(0) as u32;
        let b64 = v["rgba_b64"].as_str().unwrap_or("");
        let bytes = STANDARD.decode(b64).map_err(|e| {
            PyRuntimeError::new_err(format!("window screenshot base64 decode: {e}"))
        })?;
        Ok(Some((w, h, bytes)))
    }

    fn enqueue_set_table_data(&self, id: String, table_json: String) -> PyResult<()> {
        let parsed: serde_json::Value = serde_json::from_str(&table_json)
            .map_err(|e| PyValueError::new_err(format!("invalid table JSON: {e}")))?;
        if !parsed.is_object() {
            return Err(PyTypeError::new_err(
                "table update must serialize to a JSON object",
            ));
        }
        self.enqueue(Command::SetTableData { id, table_json })
    }

    fn enqueue_set_table_data_columns(
        &self,
        id: String,
        table_json: String,
        columns_json: String,
        buffers: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let parsed: serde_json::Value = serde_json::from_str(&table_json)
            .map_err(|e| PyValueError::new_err(format!("invalid table JSON: {e}")))?;
        if !parsed.is_object() {
            return Err(PyTypeError::new_err(
                "table update must serialize to a JSON object",
            ));
        }
        let metadata: serde_json::Value = serde_json::from_str(&columns_json)
            .map_err(|e| PyValueError::new_err(format!("invalid table columns JSON: {e}")))?;
        let metadata = metadata.as_array().ok_or_else(|| {
            PyTypeError::new_err("table column metadata must serialize to a JSON array")
        })?;
        let buffers = byte_buffers_from_py_iterable(buffers, "table column buffer")?;
        if metadata.len() != buffers.len() {
            return Err(PyValueError::new_err(
                "table column metadata and buffer counts must match",
            ));
        }
        let mut columns = Vec::with_capacity(metadata.len());
        for (meta, bytes) in metadata.iter().zip(buffers) {
            let name = meta
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PyValueError::new_err("table column metadata requires name"))?;
            let dtype = meta
                .get("dtype")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PyValueError::new_err("table column metadata requires dtype"))?;
            columns.push(TableColumnPacket {
                name: name.to_string(),
                dtype: dtype.to_string(),
                bytes,
            });
        }
        self.enqueue(Command::SetTableDataColumns {
            id,
            table_json,
            columns,
        })
    }

    #[pyo3(signature = (id, kind, data, owner_id=None))]
    fn enqueue_set_buffer_resource(
        &self,
        id: String,
        kind: String,
        data: &Bound<'_, PyAny>,
        owner_id: Option<String>,
    ) -> PyResult<()> {
        if id.trim().is_empty() {
            return Err(PyValueError::new_err("buffer resource id cannot be empty"));
        }
        if kind.trim().is_empty() {
            return Err(PyValueError::new_err(
                "buffer resource kind cannot be empty",
            ));
        }
        if owner_id.as_ref().is_some_and(|id| id.trim().is_empty()) {
            return Err(PyValueError::new_err(
                "buffer resource owner id cannot be empty",
            ));
        }
        let bytes = byte_buffer_from_py(data, "buffer resource payload")?;
        if kind == "image_encoded" {
            validate_managed_image_resource(&id, &bytes, owner_id.as_deref())?;
        }
        self.enqueue(Command::SetBufferResource {
            id,
            kind,
            bytes,
            owner_id,
        })
    }

    fn enqueue_release_resource(&self, id: String) -> PyResult<()> {
        if id.trim().is_empty() {
            return Err(PyValueError::new_err("resource id cannot be empty"));
        }
        self.enqueue(Command::ReleaseResource { id })
    }

    fn enqueue_set_stylesheet(&self, origin: String, css: String) -> PyResult<()> {
        let origin = stylesheet_origin_from_py(&origin)?;
        if css.trim().is_empty() {
            return Err(PyValueError::new_err("stylesheet CSS cannot be empty"));
        }
        parse_stylesheet(&css, origin)
            .map_err(|err| PyValueError::new_err(format!("invalid DragonGUI stylesheet: {err}")))?;
        self.enqueue(Command::SetStylesheet {
            origin,
            id: None,
            css,
        })
    }

    fn enqueue_set_named_stylesheet(
        &self,
        origin: String,
        id: String,
        css: String,
    ) -> PyResult<()> {
        let origin = stylesheet_origin_from_py(&origin)?;
        if id.trim().is_empty() {
            return Err(PyValueError::new_err(
                "stylesheet identifier cannot be empty",
            ));
        }
        if css.trim().is_empty() {
            return Err(PyValueError::new_err("stylesheet CSS cannot be empty"));
        }
        parse_stylesheet(&css, origin)
            .map_err(|err| PyValueError::new_err(format!("invalid DragonGUI stylesheet: {err}")))?;
        self.enqueue(Command::SetStylesheet {
            origin,
            id: Some(id),
            css,
        })
    }

    fn enqueue_remove_stylesheet(&self, origin: String, id: String) -> PyResult<()> {
        let origin = stylesheet_origin_from_py(&origin)?;
        if id.trim().is_empty() {
            return Err(PyValueError::new_err(
                "stylesheet identifier cannot be empty",
            ));
        }
        self.enqueue(Command::RemoveStylesheet { origin, id })
    }

    fn enqueue_set_theme(&self, theme_json: String) -> PyResult<()> {
        let theme_value: serde_json::Value = serde_json::from_str(&theme_json)
            .map_err(|err| PyValueError::new_err(format!("invalid theme JSON: {err}")))?;
        let theme = crate::document::parse_theme_from_doc(&serde_json::json!({
            "theme": theme_value
        }))
        .ok_or_else(|| PyValueError::new_err("theme must be a JSON object"))?;
        self.enqueue(Command::SetTheme { theme })
    }

    fn enqueue_set_icon_theme(&self, theme_json: String) -> PyResult<()> {
        let theme: Value = serde_json::from_str(&theme_json)
            .map_err(|err| PyValueError::new_err(format!("invalid icon theme JSON: {err}")))?;
        crate::icons::IconThemeRegistry::from_value(Some(&theme))
            .map_err(|err| PyValueError::new_err(format!("invalid icon theme: {err}")))?;
        self.enqueue(Command::SetIconTheme { theme })
    }

    fn enqueue_clear_stylesheets(&self, origin: String) -> PyResult<()> {
        let origin = stylesheet_origin_from_py(&origin)?;
        self.enqueue(Command::ClearStylesheets { origin })
    }

    #[pyo3(signature = (id, message, level, duration_ms=None, opacity=None, radius=None, padding=None, position=None))]
    fn enqueue_show_toast(
        &self,
        id: String,
        message: String,
        level: String,
        duration_ms: Option<u64>,
        opacity: Option<f32>,
        radius: Option<f32>,
        padding: Option<f32>,
        position: Option<String>,
    ) -> PyResult<()> {
        if id.trim().is_empty() {
            return Err(PyValueError::new_err("toast id cannot be empty"));
        }
        if message.trim().is_empty() {
            return Err(PyValueError::new_err("toast message cannot be empty"));
        }
        if !matches!(level.as_str(), "info" | "success" | "warning" | "error") {
            return Err(PyValueError::new_err(format!(
                "unknown toast level: {level}"
            )));
        }
        if let Some(opacity) = opacity {
            if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                return Err(PyValueError::new_err(
                    "toast opacity must be between 0.0 and 1.0",
                ));
            }
        }
        if let Some(radius) = radius {
            if !radius.is_finite() || radius < 0.0 {
                return Err(PyValueError::new_err(
                    "toast radius must be a non-negative number",
                ));
            }
        }
        if let Some(padding) = padding {
            if !padding.is_finite() || padding < 0.0 {
                return Err(PyValueError::new_err(
                    "toast padding must be a non-negative number",
                ));
            }
        }
        if let Some(position) = position.as_deref() {
            if !matches!(
                position,
                "top-right"
                    | "top_left"
                    | "top-left"
                    | "top_right"
                    | "bottom-right"
                    | "bottom_right"
                    | "bottom-left"
                    | "bottom_left"
            ) {
                return Err(PyValueError::new_err(format!(
                    "unknown toast position: {position}"
                )));
            }
        }
        self.enqueue(Command::ShowToast {
            id,
            message,
            level,
            duration_ms,
            opacity,
            radius,
            padding,
            position,
        })
    }

    fn enqueue_dismiss_toast(&self, id: String) -> PyResult<()> {
        if id.trim().is_empty() {
            return Err(PyValueError::new_err("toast id cannot be empty"));
        }
        self.enqueue(Command::DismissToast { id })
    }

    fn enqueue_drain_python_tasks(&self) -> PyResult<()> {
        self.enqueue(Command::DrainPythonTasks)
    }

    fn enqueue_request_redraw(&self) -> PyResult<()> {
        self.enqueue(Command::RequestRedraw)
    }

    fn request_window_resize(&self, width: u32, height: u32) -> PyResult<()> {
        if width == 0 || height == 0 {
            return Err(PyValueError::new_err(
                "window resize dimensions must be positive",
            ));
        }
        if self.bridge.request_window_resize(width, height) {
            Ok(())
        } else {
            Err(PyRuntimeError::new_err(
                "DragonGUI window resize could not reach the native event loop",
            ))
        }
    }

    fn enqueue_request_exit(&self) -> PyResult<()> {
        self.enqueue(Command::RequestExit)
    }

    fn is_closed(&self) -> bool {
        self.bridge.is_closed()
    }

    fn close(&self) {
        self.bridge.close();
    }

    fn queue_depth(&self) -> usize {
        self.bridge.len()
    }

    #[pyo3(signature = (timeout_ms=1000))]
    fn debug_snapshot(&self, py: Python<'_>, timeout_ms: u64) -> PyResult<String> {
        py.detach(|| {
            self.bridge
                .request_debug_snapshot(Duration::from_millis(timeout_ms))
        })
            .map_err(|err| match err {
                SnapshotError::Closed => {
                    PyRuntimeError::new_err("DragonGUI command sender is closed")
                }
                SnapshotError::Timeout => PyRuntimeError::new_err(
                    "timed out waiting for DragonGUI debug snapshot; avoid calling debug_snapshot() from a UI callback",
                ),
            })
    }

    #[pyo3(signature = (timeout_ms=1000))]
    fn latency_probe(&self, py: Python<'_>, timeout_ms: u64) -> PyResult<()> {
        py.detach(|| {
            self.bridge
                .request_latency_probe(Duration::from_millis(timeout_ms))
        })
        .map(|_| ())
        .map_err(|err| match err {
            SnapshotError::Closed => PyRuntimeError::new_err("DragonGUI command sender is closed"),
            SnapshotError::Timeout => PyRuntimeError::new_err(
                "timed out waiting for DragonGUI latency probe; avoid calling it from a UI callback",
            ),
        })
    }
}

fn now_epoch_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn stylesheet_origin_from_py(origin: &str) -> PyResult<StylesheetOrigin> {
    match origin {
        "framework" => Ok(StylesheetOrigin::Framework),
        "theme" => Ok(StylesheetOrigin::Theme),
        "user" => Ok(StylesheetOrigin::User),
        "inline" => Err(PyValueError::new_err(
            "inline styles are not stored as stylesheets",
        )),
        _ => Err(PyValueError::new_err(format!(
            "unknown stylesheet origin: {origin}"
        ))),
    }
}

fn normalize_colormap(value: Option<String>) -> String {
    let value = value
        .unwrap_or_else(|| "viridis".to_string())
        .trim()
        .to_ascii_lowercase();
    if value.is_empty() {
        "viridis".to_string()
    } else {
        value
    }
}

fn byte_buffers_from_py_iterable(
    value: &Bound<'_, PyAny>,
    context: &str,
) -> PyResult<Vec<Vec<u8>>> {
    let iter = value
        .try_iter()
        .map_err(|_| PyTypeError::new_err(format!("{context}s must be an iterable")))?;
    let mut buffers = Vec::new();
    for item in iter {
        buffers.push(byte_buffer_from_py(&item?, context)?);
    }
    Ok(buffers)
}

fn byte_buffer_from_py(value: &Bound<'_, PyAny>, context: &str) -> PyResult<Vec<u8>> {
    let buffer = PyBuffer::<u8>::get(value).map_err(|_| {
        PyTypeError::new_err(format!(
            "{context} must expose the Python buffer protocol as unsigned bytes; pass bytes, bytearray, or memoryview(...).cast('B')"
        ))
    })?;
    buffer.to_vec(value.py()).map_err(|err| {
        PyTypeError::new_err(format!(
            "failed to copy {context} into native memory: {err}"
        ))
    })
}

const MANAGED_IMAGE_MAX_ENCODED_BYTES: usize = 16 * 1024 * 1024;

fn validate_managed_image_resource(id: &str, bytes: &[u8], owner_id: Option<&str>) -> PyResult<()> {
    let valid_id = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if !valid_id {
        return Err(PyValueError::new_err(
            "image resource id must contain 1..128 ASCII letters, digits, '.', '_', or '-'",
        ));
    }
    if owner_id.is_some() {
        return Err(PyValueError::new_err(
            "managed image resources must be application-owned",
        ));
    }
    if bytes.is_empty() {
        return Err(PyValueError::new_err(
            "managed image resource cannot be empty",
        ));
    }
    if bytes.len() > MANAGED_IMAGE_MAX_ENCODED_BYTES {
        return Err(PyValueError::new_err(
            "encoded image resource cannot exceed 16 MiB",
        ));
    }
    let png = bytes.starts_with(b"\x89PNG\r\n\x1a\n");
    let jpeg = bytes.starts_with(b"\xff\xd8\xff");
    if !png && !jpeg {
        return Err(PyValueError::new_err(
            "managed image resource must contain encoded PNG or JPEG data",
        ));
    }
    Ok(())
}

fn command_value_from_py(value: &Bound<'_, PyAny>) -> PyResult<CommandValue> {
    if value.is_none() {
        return Ok(CommandValue::None);
    }
    if let Ok(v) = value.extract::<bool>() {
        return Ok(CommandValue::Bool(v));
    }
    if let Ok(v) = value.extract::<f64>() {
        return Ok(CommandValue::Float(v as f32));
    }
    if let Ok(v) = value.extract::<String>() {
        return Ok(CommandValue::Text(v));
    }
    Err(PyTypeError::new_err(
        "command values must be None, bool, float, int, or str",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_coalescing_contract_classifies_replaceable_and_lossless_commands() {
        let property = Command::SetProp {
            id: "status".to_string(),
            prop: "text".to_string(),
            value: CommandValue::Text("ready".to_string()),
        };
        assert_eq!(
            property.coalescing_key(),
            Some(CommandCoalescingKey::SetProp(
                "status".to_string(),
                "text".to_string()
            ))
        );
        assert_eq!(
            PropUpdate {
                id: "status".to_string(),
                prop: "text".to_string(),
                value: CommandValue::Text("packet".to_string()),
            }
            .coalescing_key(),
            CommandCoalescingKey::SetProp("status".to_string(), "text".to_string())
        );

        let coalesced_scatter = Command::SetScatterPointsPacked {
            id: "scatter".to_string(),
            xyz: vec![0; 12],
            telemetry: None,
            colormap: "viridis".to_string(),
            payload_format: ScatterPayloadFormat::XyzF32V0,
            fit: false,
            coalesce: true,
        };
        assert_eq!(
            coalesced_scatter.coalescing_key(),
            Some(CommandCoalescingKey::ScatterPoints("scatter".to_string()))
        );
        assert_eq!(
            Command::UpdateExtensionDisplayList {
                id: "scope".to_string(),
                display_list_json: "[]".to_string(),
            }
            .coalescing_key(),
            Some(CommandCoalescingKey::ExtensionDisplayList(
                "scope".to_string()
            ))
        );
        assert_eq!(
            Command::SetIconTheme {
                theme: serde_json::json!({"search": "help"}),
            }
            .coalescing_key(),
            Some(CommandCoalescingKey::IconTheme)
        );

        let mut lossless_scatter = coalesced_scatter.clone();
        if let Command::SetScatterPointsPacked { coalesce, .. } = &mut lossless_scatter {
            *coalesce = false;
        }
        let lossless_commands = [
            Command::SetProps {
                updates: vec![PropUpdate {
                    id: "status".to_string(),
                    prop: "text".to_string(),
                    value: CommandValue::Text("packet".to_string()),
                }],
            },
            lossless_scatter,
            Command::AppendLinePlotPointsPacked {
                id: "line".to_string(),
                series: "value".to_string(),
                xy: vec![0; 8],
                max_points: None,
                payload_format: LinePlotPayloadFormat::XyF32V0,
            },
            Command::ReplaceNode {
                id: "status".to_string(),
                node_json: "{}".to_string(),
            },
            Command::DebugSnapshot { request_id: 1 },
        ];
        assert!(lossless_commands
            .iter()
            .all(|command| command.coalescing_key().is_none()));
        assert_eq!(
            lossless_commands[3].coalescing_barrier(),
            Some(CommandCoalescingBarrier::Structural)
        );
        assert_eq!(
            lossless_commands[4].coalescing_barrier(),
            Some(CommandCoalescingBarrier::Observation)
        );
        assert_eq!(
            Command::DrainPythonTasks.coalescing_barrier(),
            Some(CommandCoalescingBarrier::Callback)
        );
        assert!(lossless_commands[..3]
            .iter()
            .all(|command| command.coalescing_barrier().is_none()));
    }

    #[test]
    fn command_coalescing_contract_merges_sticky_flags_only_with_matching_families() {
        let mut latest_scatter = Command::SetScatterPointsPacked {
            id: "scatter".to_string(),
            xyz: vec![2; 12],
            telemetry: None,
            colormap: "turbo".to_string(),
            payload_format: ScatterPayloadFormat::XyzF32V0,
            fit: false,
            coalesce: true,
        };
        let previous_scatter = Command::SetScatterPointsPacked {
            id: "scatter".to_string(),
            xyz: vec![1; 12],
            telemetry: None,
            colormap: "viridis".to_string(),
            payload_format: ScatterPayloadFormat::XyzF32V0,
            fit: true,
            coalesce: true,
        };
        latest_scatter.merge_replaced(&previous_scatter);
        assert!(matches!(
            latest_scatter,
            Command::SetScatterPointsPacked { fit: true, .. }
        ));

        let mut latest_histogram = Command::SetHistogramData {
            id: "histogram".to_string(),
            edges: vec![0.0, 1.0],
            counts: vec![2.0],
            input_count: 2,
            finite_count: 2,
            auto_fit: false,
            coalesce: true,
        };
        let previous_histogram = Command::SetHistogramData {
            id: "histogram".to_string(),
            edges: vec![0.0, 1.0],
            counts: vec![1.0],
            input_count: 1,
            finite_count: 1,
            auto_fit: true,
            coalesce: true,
        };
        latest_histogram.merge_replaced(&previous_histogram);
        assert!(matches!(
            latest_histogram,
            Command::SetHistogramData { auto_fit: true, .. }
        ));
    }

    #[test]
    fn command_coalescing_contract_merges_only_compatible_adjacent_appends() {
        let mut first = Command::AppendLinePlotPointsPacked {
            id: "line".to_string(),
            series: "temperature".to_string(),
            xy: vec![1; 8],
            max_points: Some(1024),
            payload_format: LinePlotPayloadFormat::XyF32V0,
        };
        let second = Command::AppendLinePlotPointsPacked {
            id: "line".to_string(),
            series: "temperature".to_string(),
            xy: vec![2; 16],
            max_points: Some(1024),
            payload_format: LinePlotPayloadFormat::XyF32V0,
        };
        assert!(first.try_merge_adjacent(second).is_ok());
        assert!(matches!(
            &first,
            Command::AppendLinePlotPointsPacked { xy, .. }
                if xy.len() == 24 && xy[..8] == [1; 8] && xy[8..] == [2; 16]
        ));

        let incompatible = Command::AppendLinePlotPointsPacked {
            id: "line".to_string(),
            series: "pressure".to_string(),
            xy: vec![3; 8],
            max_points: Some(1024),
            payload_format: LinePlotPayloadFormat::XyF32V0,
        };
        assert!(first.try_merge_adjacent(incompatible).is_err());
        assert!(matches!(
            first,
            Command::AppendLinePlotPointsPacked { xy, .. } if xy.len() == 24
        ));
    }

    #[test]
    fn managed_image_resource_validation_rejects_unsafe_bridge_payloads() {
        let png = b"\x89PNG\r\n\x1a\npayload";
        assert!(validate_managed_image_resource("surface.tile", png, None).is_ok());
        assert!(validate_managed_image_resource("../escape", png, None).is_err());
        assert!(validate_managed_image_resource("surface", png, Some("owner")).is_err());
        assert!(validate_managed_image_resource("surface", b"GIF89a", None).is_err());
        assert!(validate_managed_image_resource("surface", b"", None).is_err());
        assert_eq!(MANAGED_IMAGE_MAX_ENCODED_BYTES, 16 * 1024 * 1024);
    }

    #[test]
    fn queue_push_drain_preserves_order() {
        let queue = CommandQueue::default();

        queue
            .push(Command::Invalidate {
                id: "a".to_string(),
                dirty: Dirty::Visual,
            })
            .unwrap();
        queue
            .push(Command::SetProp {
                id: "b".to_string(),
                prop: "value".to_string(),
                value: CommandValue::Float(0.5),
            })
            .unwrap();
        queue
            .push(Command::SetStyle {
                id: "c".to_string(),
                patch_json: r#"{"background":"accent"}"#.to_string(),
            })
            .unwrap();
        queue
            .push(Command::ReplaceChildren {
                id: "panel".to_string(),
                children_json: r#"[{"id":"label","type":"label","props":{"text":"ok"}}]"#
                    .to_string(),
            })
            .unwrap();
        queue
            .push(Command::ReplaceNode {
                id: "old".to_string(),
                node_json: r#"{"id":"new","type":"button","props":{"text":"ok"}}"#.to_string(),
            })
            .unwrap();
        queue
            .push(Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![0; 12],
                telemetry: None,
                colormap: "viridis".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                fit: false,
                coalesce: true,
            })
            .unwrap();
        queue
            .push(Command::SetTableData {
                id: "table".to_string(),
                table_json: r#"{"frame":{"columns":["x"],"dtypes":["f32"],"rows":1},"page_size":10,"cells":[["1"]]}"#.to_string(),
            })
            .unwrap();
        queue
            .push(Command::SetBufferResource {
                id: "buffer".to_string(),
                kind: "bytes".to_string(),
                bytes: vec![1, 2, 3],
                owner_id: None,
            })
            .unwrap();
        queue
            .push(Command::ReleaseResource {
                id: "buffer".to_string(),
            })
            .unwrap();
        queue
            .push(Command::SetStylesheet {
                origin: StylesheetOrigin::User,
                id: None,
                css: "Button { border-radius: 4px; }".to_string(),
            })
            .unwrap();
        queue
            .push(Command::ClearStylesheets {
                origin: StylesheetOrigin::User,
            })
            .unwrap();
        queue
            .push(Command::DebugSnapshot { request_id: 7 })
            .unwrap();

        assert_eq!(queue.len(), 11);
        assert_eq!(
            queue.drain(),
            vec![
                Command::Invalidate {
                    id: "a".to_string(),
                    dirty: Dirty::Visual,
                },
                Command::SetProp {
                    id: "b".to_string(),
                    prop: "value".to_string(),
                    value: CommandValue::Float(0.5),
                },
                Command::SetStyle {
                    id: "c".to_string(),
                    patch_json: r#"{"background":"accent"}"#.to_string(),
                },
                Command::ReplaceChildren {
                    id: "panel".to_string(),
                    children_json: r#"[{"id":"label","type":"label","props":{"text":"ok"}}]"#
                        .to_string(),
                },
                Command::ReplaceNode {
                    id: "old".to_string(),
                    node_json: r#"{"id":"new","type":"button","props":{"text":"ok"}}"#
                        .to_string(),
                },
                Command::SetScatterPointsPacked {
                    id: "scatter".to_string(),
                    xyz: vec![0; 12],
                    telemetry: None,
                    colormap: "viridis".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    fit: false,
                    coalesce: true,
                },
                Command::SetTableData {
                    id: "table".to_string(),
                    table_json: r#"{"frame":{"columns":["x"],"dtypes":["f32"],"rows":1},"page_size":10,"cells":[["1"]]}"#.to_string(),
                },
                Command::SetBufferResource {
                    id: "buffer".to_string(),
                    kind: "bytes".to_string(),
                    bytes: vec![1, 2, 3],
                    owner_id: None,
                },
                Command::ReleaseResource {
                    id: "buffer".to_string(),
                },
                Command::ClearStylesheets {
                    origin: StylesheetOrigin::User,
                },
                Command::DebugSnapshot { request_id: 7 },
            ]
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn queue_preserves_property_packet_as_one_ordered_command() {
        let queue = CommandQueue::default();
        let updates = vec![
            PropUpdate {
                id: "first".to_string(),
                prop: "text".to_string(),
                value: CommandValue::Text("one".to_string()),
            },
            PropUpdate {
                id: "second".to_string(),
                prop: "value".to_string(),
                value: CommandValue::Float(0.5),
            },
        ];

        queue
            .push(Command::SetProps {
                updates: updates.clone(),
            })
            .unwrap();

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.debug_snapshot()["pushes"], 1);
        assert_eq!(
            queue.drain(),
            vec![Command::SetProps {
                updates: updates.clone(),
            }]
        );
    }

    #[test]
    fn queue_coalesces_pending_theme_and_stylesheet_replacements() {
        let queue = CommandQueue::default();
        let mut first_theme = Theme::dark();
        first_theme.spacing = 4.0;
        let mut final_theme = Theme::dark();
        final_theme.spacing = 9.0;

        queue
            .push(Command::SetTheme { theme: first_theme })
            .unwrap();
        queue
            .push(Command::SetStylesheet {
                origin: StylesheetOrigin::User,
                id: Some("appearance".to_string()),
                css: "Button { color: red; }".to_string(),
            })
            .unwrap();
        queue
            .push(Command::SetTheme { theme: final_theme })
            .unwrap();
        queue
            .push(Command::SetStylesheet {
                origin: StylesheetOrigin::User,
                id: Some("appearance".to_string()),
                css: "Button { color: blue; }".to_string(),
            })
            .unwrap();

        let commands = queue.drain();
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[0],
            Command::SetTheme { theme } if theme.spacing == 9.0
        ));
        assert!(matches!(
            &commands[1],
            Command::SetStylesheet { id, css, .. }
                if id.as_deref() == Some("appearance") && css.contains("blue")
        ));
    }

    #[test]
    fn queue_keeps_only_the_latest_pending_widget_property_value() {
        let queue = CommandQueue::default();
        for value in ["first", "second", "final"] {
            queue
                .push(Command::SetProp {
                    id: "theme-status".to_string(),
                    prop: "text".to_string(),
                    value: CommandValue::Text(value.to_string()),
                })
                .unwrap();
        }
        queue
            .push(Command::SetProp {
                id: "theme-status".to_string(),
                prop: "class".to_string(),
                value: CommandValue::Text("active".to_string()),
            })
            .unwrap();

        let commands = queue.drain();
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[0],
            Command::SetProp {
                prop,
                value: CommandValue::Text(value),
                ..
            } if prop == "text" && value == "final"
        ));
        assert!(matches!(
            &commands[1],
            Command::SetProp {
                prop,
                value: CommandValue::Text(value),
                ..
            } if prop == "class" && value == "active"
        ));
    }

    #[test]
    fn queue_debug_snapshot_counts_pushes_replacements_and_high_water() {
        let queue = CommandQueue::default();
        for value in ["first", "final"] {
            queue
                .push(Command::SetProp {
                    id: "status".to_string(),
                    prop: "text".to_string(),
                    value: CommandValue::Text(value.to_string()),
                })
                .unwrap();
        }
        queue
            .push(Command::SetProp {
                id: "status".to_string(),
                prop: "class".to_string(),
                value: CommandValue::Text("active".to_string()),
            })
            .unwrap();
        queue
            .push(Command::Invalidate {
                id: "status".to_string(),
                dirty: Dirty::Visual,
            })
            .unwrap();

        let snapshot = queue.debug_snapshot();
        assert_eq!(snapshot["depth"], 3);
        assert_eq!(snapshot["high_water"], 3);
        assert_eq!(snapshot["pushes"], 4);
        assert_eq!(snapshot["replacements"], 1);
        assert_eq!(snapshot["replacements_by_family"]["set_prop"], 1);
        assert_eq!(snapshot["push_timing"]["count"], 4);
        assert_eq!(snapshot["push_timing"]["sample_window"], 4);
        assert!(snapshot["push_timing"]["max_ms"].as_f64().unwrap() >= 0.0);
        assert_eq!(snapshot["live_entries"], 3);
        assert_eq!(snapshot["physical_slots"], 3);
        assert_eq!(snapshot["free_slots"], 0);
        assert_eq!(snapshot["stale_entries"], 0);
        assert_eq!(snapshot["compactions"], 0);
        assert_eq!(snapshot["peak_physical_queue_length"], 3);
    }

    #[test]
    fn queue_replacement_does_not_cross_an_observation_barrier() {
        let queue = CommandQueue::default();
        queue
            .push(Command::SetProp {
                id: "a".to_string(),
                prop: "text".to_string(),
                value: CommandValue::Text("first".to_string()),
            })
            .unwrap();
        queue
            .push(Command::DebugSnapshot { request_id: 17 })
            .unwrap();
        queue
            .push(Command::SetProp {
                id: "b".to_string(),
                prop: "text".to_string(),
                value: CommandValue::Text("middle".to_string()),
            })
            .unwrap();
        queue
            .push(Command::SetProp {
                id: "a".to_string(),
                prop: "text".to_string(),
                value: CommandValue::Text("final".to_string()),
            })
            .unwrap();

        let drained = queue.drain();
        assert!(matches!(
            &drained[0],
            Command::SetProp { id, value: CommandValue::Text(value), .. }
                if id == "a" && value == "first"
        ));
        assert!(matches!(
            drained[1],
            Command::DebugSnapshot { request_id: 17 }
        ));
        assert!(matches!(&drained[2], Command::SetProp { id, .. } if id == "b"));
        assert!(matches!(
            &drained[3],
            Command::SetProp { id, value: CommandValue::Text(value), .. }
                if id == "a" && value == "final"
        ));
    }

    #[test]
    fn queue_replacement_does_not_cross_a_structural_barrier() {
        let queue = CommandQueue::default();
        queue
            .push(Command::SetProp {
                id: "status".to_string(),
                prop: "text".to_string(),
                value: CommandValue::Text("before".to_string()),
            })
            .unwrap();
        queue
            .push(Command::ReplaceNode {
                id: "status".to_string(),
                node_json: r#"{"id":"status","type":"label"}"#.to_string(),
            })
            .unwrap();
        queue
            .push(Command::SetProp {
                id: "status".to_string(),
                prop: "text".to_string(),
                value: CommandValue::Text("after".to_string()),
            })
            .unwrap();

        let drained = queue.drain();
        assert_eq!(drained.len(), 3);
        assert!(matches!(
            &drained[0],
            Command::SetProp { value: CommandValue::Text(value), .. } if value == "before"
        ));
        assert!(matches!(drained[1], Command::ReplaceNode { .. }));
        assert!(matches!(
            &drained[2],
            Command::SetProp { value: CommandValue::Text(value), .. } if value == "after"
        ));
    }

    #[test]
    fn queue_uses_shared_keys_for_display_lists_and_icon_themes() {
        let queue = CommandQueue::default();
        queue
            .push(Command::UpdateExtensionDisplayList {
                id: "scope".to_string(),
                display_list_json: r#"[{"cmd":"old"}]"#.to_string(),
            })
            .unwrap();
        queue
            .push(Command::SetIconTheme {
                theme: serde_json::json!({"search": "help"}),
            })
            .unwrap();
        queue
            .push(Command::UpdateExtensionDisplayList {
                id: "scope".to_string(),
                display_list_json: r#"[{"cmd":"new"}]"#.to_string(),
            })
            .unwrap();
        queue
            .push(Command::SetIconTheme {
                theme: serde_json::json!({"search": "warning"}),
            })
            .unwrap();

        let snapshot = queue.debug_snapshot();
        assert_eq!(snapshot["replacements"], 2);
        assert_eq!(
            snapshot["replacements_by_family"]["extension_display_list"],
            1
        );
        assert_eq!(snapshot["replacements_by_family"]["icon_theme"], 1);

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(matches!(
            &drained[0],
            Command::UpdateExtensionDisplayList { display_list_json, .. }
                if display_list_json.contains("new")
        ));
        assert!(matches!(
            &drained[1],
            Command::SetIconTheme { theme } if theme["search"] == "warning"
        ));
    }

    #[test]
    fn linked_queue_reuses_physical_slots_under_a_large_backlog() {
        let queue = CommandQueue::default();
        for index in 0..10_000 {
            queue
                .push(Command::SetProp {
                    id: format!("widget-{index}"),
                    prop: "text".to_string(),
                    value: CommandValue::Float(index as f32),
                })
                .unwrap();
        }
        for value in 0..10_000 {
            queue
                .push(Command::SetProp {
                    id: "widget-0".to_string(),
                    prop: "text".to_string(),
                    value: CommandValue::Float(value as f32),
                })
                .unwrap();
        }

        let snapshot = queue.debug_snapshot();
        assert_eq!(snapshot["depth"], 10_000);
        assert_eq!(snapshot["physical_slots"], 10_000);
        assert_eq!(snapshot["free_slots"], 0);
        assert_eq!(snapshot["replacements_by_family"]["set_prop"], 10_000);

        let mut first_half = Vec::new();
        queue.drain_limited_into(&mut first_half, 5_000);
        assert_eq!(first_half.len(), 5_000);
        let half_snapshot = queue.debug_snapshot();
        assert_eq!(half_snapshot["depth"], 5_000);
        assert_eq!(half_snapshot["physical_slots"], 10_000);
        assert_eq!(half_snapshot["free_slots"], 5_000);

        for _ in 0..5_000 {
            queue.push(Command::DrainPythonTasks).unwrap();
        }
        let reused_snapshot = queue.debug_snapshot();
        assert_eq!(reused_snapshot["depth"], 10_000);
        assert_eq!(reused_snapshot["physical_slots"], 10_000);
        assert_eq!(reused_snapshot["free_slots"], 0);
    }

    #[test]
    fn queue_close_rejects_new_commands_but_preserves_existing() {
        let queue = CommandQueue::default();
        queue.push(Command::DrainPythonTasks).unwrap();
        queue.close();

        assert_eq!(
            queue.push(Command::DrainPythonTasks),
            Err(CommandQueueError::Closed)
        );
        assert_eq!(queue.drain(), vec![Command::DrainPythonTasks]);
    }

    #[test]
    fn native_sender_validates_icon_theme_before_enqueue() {
        let bridge = Arc::new(CommandBridge::new());
        let sender = NativeCommandSender::new(Arc::clone(&bridge));
        sender
            .enqueue_set_icon_theme(
                r#"{"search":{"type":"stroke","view_box":[0,0,24,24],"stroke_width":2,"strokes":[{"points":[[3,3],[21,21]]}]}}"#
                    .to_string(),
            )
            .unwrap();
        assert!(matches!(
            bridge.drain().as_slice(),
            [Command::SetIconTheme { theme }] if theme["search"]["type"] == "stroke"
        ));
        assert!(sender
            .enqueue_set_icon_theme(r#"{"search":"missing-terminal"}"#.to_string())
            .is_err());
        assert!(bridge.is_empty());
    }

    #[test]
    fn dirty_parse_accepts_expected_names() {
        assert_eq!(Dirty::from_str("layout"), Some(Dirty::Layout));
        assert_eq!(Dirty::from_str("Text"), Some(Dirty::Text));
        assert_eq!(Dirty::from_str("gpuData"), Some(Dirty::GpuData));
        assert_eq!(Dirty::from_str("unknown"), None);
    }

    #[test]
    fn bridge_push_drain_uses_shared_queue() {
        let bridge = CommandBridge::new();
        bridge.push(Command::DrainPythonTasks).unwrap();

        assert_eq!(bridge.len(), 1);
        assert_eq!(bridge.drain(), vec![Command::DrainPythonTasks]);
        assert!(bridge.is_empty());
    }

    #[test]
    fn queue_drain_into_reuses_output_vec() {
        let queue = CommandQueue::default();
        let mut out = Vec::with_capacity(4);

        queue.push(Command::DrainPythonTasks).unwrap();
        queue
            .push(Command::Invalidate {
                id: "a".to_string(),
                dirty: Dirty::Visual,
            })
            .unwrap();

        queue.drain_into(&mut out);
        assert_eq!(
            out,
            vec![
                Command::DrainPythonTasks,
                Command::Invalidate {
                    id: "a".to_string(),
                    dirty: Dirty::Visual,
                }
            ]
        );
        assert!(queue.is_empty());

        out.clear();
        queue.drain_into(&mut out);
        assert!(out.is_empty());
        assert!(out.capacity() >= 4);
    }

    #[test]
    fn queue_preserves_scatter_updates_across_callback_barrier() {
        let queue = CommandQueue::default();

        queue
            .push(Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![1; 12],
                telemetry: None,
                colormap: "viridis".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                fit: true,
                coalesce: true,
            })
            .unwrap();
        queue.push(Command::DrainPythonTasks).unwrap();
        queue
            .push(Command::SetScatterPointsPacked {
                id: "other".to_string(),
                xyz: vec![2; 12],
                telemetry: None,
                colormap: "viridis".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                fit: false,
                coalesce: true,
            })
            .unwrap();
        queue
            .push(Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![3; 12],
                telemetry: None,
                colormap: "turbo".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                fit: false,
                coalesce: true,
            })
            .unwrap();

        let snapshot = queue.debug_snapshot();
        assert_eq!(snapshot["barrier_segments"]["total"], 1);
        assert_eq!(snapshot["barrier_segments"]["by_class"]["callback"], 1);
        assert_eq!(
            queue.drain(),
            vec![
                Command::SetScatterPointsPacked {
                    id: "scatter".to_string(),
                    xyz: vec![1; 12],
                    telemetry: None,
                    colormap: "viridis".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    fit: true,
                    coalesce: true,
                },
                Command::DrainPythonTasks,
                Command::SetScatterPointsPacked {
                    id: "other".to_string(),
                    xyz: vec![2; 12],
                    telemetry: None,
                    colormap: "viridis".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    fit: false,
                    coalesce: true,
                },
                Command::SetScatterPointsPacked {
                    id: "scatter".to_string(),
                    xyz: vec![3; 12],
                    telemetry: None,
                    colormap: "turbo".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    fit: false,
                    coalesce: true,
                },
            ]
        );
    }

    #[test]
    fn queue_removes_all_pending_coalesced_scatter_updates_by_widget() {
        let queue = CommandQueue::default();

        for value in [1_u8, 2, 3] {
            queue
                .push(Command::SetScatterPointsPacked {
                    id: "scatter".to_string(),
                    xyz: vec![value; 12],
                    telemetry: None,
                    colormap: "viridis".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    fit: value == 1,
                    coalesce: true,
                })
                .unwrap();
        }

        assert_eq!(
            queue.drain(),
            vec![Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![3; 12],
                telemetry: None,
                colormap: "viridis".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                fit: true,
                coalesce: true,
            }]
        );
    }

    #[test]
    fn queue_preserves_scatter_scalar_bars_across_callback_barrier() {
        let queue = CommandQueue::default();

        queue
            .push(Command::SetScatterScalarBar {
                id: "scatter".to_string(),
                visible: true,
                vmin: 0.0,
                vmax: 1.0,
                log_scale: false,
                colormap: "turbo".to_string(),
                title: Some("z".to_string()),
            })
            .unwrap();
        queue.push(Command::DrainPythonTasks).unwrap();
        queue
            .push(Command::SetScatterScalarBar {
                id: "scatter".to_string(),
                visible: true,
                vmin: -1.0,
                vmax: 2.0,
                log_scale: false,
                colormap: "viridis".to_string(),
                title: Some("z".to_string()),
            })
            .unwrap();

        assert_eq!(
            queue.drain(),
            vec![
                Command::SetScatterScalarBar {
                    id: "scatter".to_string(),
                    visible: true,
                    vmin: 0.0,
                    vmax: 1.0,
                    log_scale: false,
                    colormap: "turbo".to_string(),
                    title: Some("z".to_string()),
                },
                Command::DrainPythonTasks,
                Command::SetScatterScalarBar {
                    id: "scatter".to_string(),
                    visible: true,
                    vmin: -1.0,
                    vmax: 2.0,
                    log_scale: false,
                    colormap: "viridis".to_string(),
                    title: Some("z".to_string()),
                },
            ]
        );
    }

    #[test]
    fn queue_preserves_noncoalesced_scatter_updates() {
        let queue = CommandQueue::default();

        for value in [1_u8, 2, 3] {
            queue
                .push(Command::SetScatterPointsPacked {
                    id: "scatter".to_string(),
                    xyz: vec![value; 12],
                    telemetry: None,
                    colormap: "viridis".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    fit: false,
                    coalesce: false,
                })
                .unwrap();
        }

        assert_eq!(queue.drain().len(), 3);
    }

    #[test]
    fn queue_preserves_scatter_updates_across_debug_snapshot() {
        let queue = CommandQueue::default();

        queue
            .push(Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![1; 12],
                telemetry: None,
                colormap: "viridis".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                fit: false,
                coalesce: true,
            })
            .unwrap();
        queue
            .push(Command::DebugSnapshot { request_id: 9 })
            .unwrap();
        queue
            .push(Command::SetScatterPointsPacked {
                id: "scatter".to_string(),
                xyz: vec![2; 12],
                telemetry: None,
                colormap: "turbo".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                fit: false,
                coalesce: true,
            })
            .unwrap();

        let commands = queue.drain();
        assert_eq!(commands.len(), 3);
        assert!(matches!(
            &commands[0],
            Command::SetScatterPointsPacked { xyz, colormap, .. }
                if xyz == &vec![1; 12] && colormap == "viridis"
        ));
        assert!(matches!(
            commands[1],
            Command::DebugSnapshot { request_id: 9 }
        ));
        match &commands[2] {
            Command::SetScatterPointsPacked { xyz, colormap, .. } => {
                assert_eq!(xyz, &vec![2; 12]);
                assert_eq!(colormap, "turbo");
            }
            other => panic!("expected latest scatter update, got {other:?}"),
        }
    }

    #[test]
    fn queue_coalesces_pending_scatter_actor_updates_by_actor() {
        let queue = CommandQueue::default();

        for value in [1_u8, 2, 3] {
            queue
                .push(Command::UpdateScatterActorPacked {
                    id: "scatter".to_string(),
                    actor_id: 7,
                    payload: vec![value; 12],
                    colormap: "viridis".to_string(),
                    payload_format: ScatterPayloadFormat::XyzF32V0,
                    tooltip_axis_labels: ["x".to_string(), "y".to_string(), "z".to_string()],
                })
                .unwrap();
        }
        queue
            .push(Command::UpdateScatterActorPacked {
                id: "scatter".to_string(),
                actor_id: 8,
                payload: vec![9; 12],
                colormap: "turbo".to_string(),
                payload_format: ScatterPayloadFormat::XyzF32V0,
                tooltip_axis_labels: ["x".to_string(), "y".to_string(), "z".to_string()],
            })
            .unwrap();

        let commands = queue.drain();
        assert_eq!(commands.len(), 2);
        match &commands[0] {
            Command::UpdateScatterActorPacked {
                actor_id, payload, ..
            } => {
                assert_eq!(*actor_id, 7);
                assert_eq!(payload, &vec![3; 12]);
            }
            other => panic!("expected actor update, got {other:?}"),
        }
        match &commands[1] {
            Command::UpdateScatterActorPacked {
                actor_id, payload, ..
            } => {
                assert_eq!(*actor_id, 8);
                assert_eq!(payload, &vec![9; 12]);
            }
            other => panic!("expected actor update, got {other:?}"),
        }
    }

    #[test]
    fn debug_snapshot_request_completes() {
        let bridge = Arc::new(CommandBridge::new());
        let worker = Arc::clone(&bridge);
        let handle = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(500);
            let commands = loop {
                let commands = worker.drain();
                if !commands.is_empty() {
                    break commands;
                }
                assert!(
                    Instant::now() < deadline,
                    "snapshot command was not enqueued"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(commands, vec![Command::DebugSnapshot { request_id: 0 }]);
            worker.complete_response(0, r#"{"status":"ok"}"#.to_string());
        });

        let snapshot = bridge
            .request_debug_snapshot(Duration::from_millis(500))
            .unwrap();
        handle.join().unwrap();
        assert_eq!(snapshot, r#"{"status":"ok"}"#);
    }

    #[test]
    fn latency_probe_request_completes_without_snapshot_payload() {
        let bridge = Arc::new(CommandBridge::new());
        let worker = Arc::clone(&bridge);
        let handle = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(500);
            let commands = loop {
                let commands = worker.drain();
                if !commands.is_empty() {
                    break commands;
                }
                assert!(
                    Instant::now() < deadline,
                    "latency probe command was not enqueued"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(commands, vec![Command::LatencyProbe { request_id: 0 }]);
            worker.complete_response(0, "{}".to_string());
        });

        let response = bridge
            .request_latency_probe(Duration::from_millis(500))
            .unwrap();
        handle.join().unwrap();
        assert_eq!(response, "{}");
    }
}
