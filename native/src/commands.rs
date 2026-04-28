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
use winit::event_loop::EventLoopProxy;

use crate::css_style::{parse_stylesheet, StylesheetOrigin};

/// User event sent into the winit loop when the Python/Rust runtime bridge has
/// work waiting.  Keep this small and cloneable; all payloads stay in the
/// command queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEvent {
    Wake,
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

/// Native runtime command consumed by the winit UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetProp {
        id: String,
        prop: String,
        value: CommandValue,
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
    SetScatterPointsPacked {
        id: String,
        xyz: Vec<u8>,
        telemetry: Option<ScatterTelemetry>,
        colormap: String,
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
        css: String,
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
    DebugSnapshot {
        request_id: u64,
    },
    DrainPythonTasks,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScatterTelemetry {
    pub pack_ms: f64,
    pub enqueue_epoch_ms: f64,
    pub point_count: usize,
    pub payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnPacket {
    pub name: String,
    pub dtype: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandQueueError {
    Closed,
}

#[derive(Debug, Default)]
struct CommandQueueInner {
    items: VecDeque<Command>,
}

/// Thread-safe command queue shared by Python-facing senders and the future UI
/// thread bridge.
#[derive(Debug, Default)]
pub struct CommandQueue {
    closed: AtomicBool,
    inner: Mutex<CommandQueueInner>,
}

impl CommandQueue {
    pub fn push(&self, command: Command) -> Result<(), CommandQueueError> {
        if self.is_closed() {
            return Err(CommandQueueError::Closed);
        }
        self.inner
            .lock()
            .expect("command queue mutex poisoned")
            .items
            .push_back(command);
        Ok(())
    }

    pub fn drain(&self) -> Vec<Command> {
        let mut inner = self.inner.lock().expect("command queue mutex poisoned");
        inner.items.drain(..).collect()
    }

    pub fn drain_into(&self, out: &mut Vec<Command>) {
        let mut inner = self.inner.lock().expect("command queue mutex poisoned");
        out.extend(inner.items.drain(..));
    }

    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("command queue mutex poisoned")
            .items
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
    snapshot_seq: AtomicU64,
    snapshots: Mutex<HashMap<u64, Option<String>>>,
    snapshot_cv: Condvar,
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

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn close(&self) {
        self.queue.close();
        self.proxy
            .lock()
            .expect("command bridge proxy mutex poisoned")
            .take();
        self.snapshot_cv.notify_all();
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
        let proxy = self
            .proxy
            .lock()
            .expect("command bridge proxy mutex poisoned")
            .clone();
        if let Some(proxy) = proxy {
            let _ = proxy.send_event(RuntimeEvent::Wake);
        }
    }

    pub fn request_debug_snapshot(&self, timeout: Duration) -> Result<String, SnapshotError> {
        if self.is_closed() {
            return Err(SnapshotError::Closed);
        }

        let request_id = self.snapshot_seq.fetch_add(1, Ordering::Relaxed);
        {
            let mut snapshots = self
                .snapshots
                .lock()
                .expect("command bridge snapshot mutex poisoned");
            snapshots.insert(request_id, None);
        }

        if self.push(Command::DebugSnapshot { request_id }).is_err() {
            self.snapshots
                .lock()
                .expect("command bridge snapshot mutex poisoned")
                .remove(&request_id);
            return Err(SnapshotError::Closed);
        }

        let deadline = Instant::now() + timeout;
        let mut snapshots = self
            .snapshots
            .lock()
            .expect("command bridge snapshot mutex poisoned");
        loop {
            if snapshots
                .get(&request_id)
                .and_then(|slot| slot.as_ref())
                .is_some()
            {
                let snapshot = snapshots
                    .remove(&request_id)
                    .and_then(|slot| slot)
                    .expect("snapshot response disappeared");
                return Ok(snapshot);
            }
            if self.is_closed() {
                snapshots.remove(&request_id);
                return Err(SnapshotError::Closed);
            }

            let now = Instant::now();
            if now >= deadline {
                snapshots.remove(&request_id);
                return Err(SnapshotError::Timeout);
            }
            let wait = deadline.saturating_duration_since(now);
            let (next, timeout_result) = self
                .snapshot_cv
                .wait_timeout(snapshots, wait)
                .expect("command bridge snapshot condvar poisoned");
            snapshots = next;
            if timeout_result.timed_out() {
                snapshots.remove(&request_id);
                return Err(SnapshotError::Timeout);
            }
        }
    }

    pub fn complete_debug_snapshot(&self, request_id: u64, snapshot_json: String) {
        let mut snapshots = self
            .snapshots
            .lock()
            .expect("command bridge snapshot mutex poisoned");
        if let Some(slot) = snapshots.get_mut(&request_id) {
            *slot = Some(snapshot_json);
            self.snapshot_cv.notify_all();
        }
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

    fn enqueue_set_prop(&self, id: String, prop: String, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.enqueue(Command::SetProp {
            id,
            prop,
            value: command_value_from_py(value)?,
        })
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

    fn enqueue_invalidate(&self, id: String, dirty: String) -> PyResult<()> {
        let dirty = Dirty::from_str(&dirty)
            .ok_or_else(|| PyValueError::new_err(format!("unknown dirty flag: {dirty}")))?;
        self.enqueue(Command::Invalidate { id, dirty })
    }

    #[pyo3(signature = (id, xyz, pack_ms=None, enqueue_epoch_ms=None, colormap=None))]
    fn enqueue_set_scatter_points_packed(
        &self,
        id: String,
        xyz: &Bound<'_, PyAny>,
        pack_ms: Option<f64>,
        enqueue_epoch_ms: Option<f64>,
        colormap: Option<String>,
    ) -> PyResult<()> {
        let xyz = byte_buffer_from_py(xyz, "scatter point payload")?;
        let point_count = xyz.len() / 12;
        let payload_bytes = xyz.len();
        let telemetry = Some(ScatterTelemetry {
            pack_ms: pack_ms.unwrap_or(0.0).max(0.0),
            enqueue_epoch_ms: enqueue_epoch_ms.unwrap_or_else(now_epoch_ms),
            point_count,
            payload_bytes,
        });
        self.enqueue(Command::SetScatterPointsPacked {
            id,
            xyz,
            telemetry,
            colormap: normalize_colormap(colormap),
        })
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
        self.enqueue(Command::SetStylesheet { origin, css })
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
    fn debug_snapshot(&self, timeout_ms: u64) -> PyResult<String> {
        self.bridge
            .request_debug_snapshot(Duration::from_millis(timeout_ms))
            .map_err(|err| match err {
                SnapshotError::Closed => {
                    PyRuntimeError::new_err("DragonGUI command sender is closed")
                }
                SnapshotError::Timeout => PyRuntimeError::new_err(
                    "timed out waiting for DragonGUI debug snapshot; avoid calling debug_snapshot() from a UI callback",
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

        assert_eq!(queue.len(), 12);
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
                Command::SetStylesheet {
                    origin: StylesheetOrigin::User,
                    css: "Button { border-radius: 4px; }".to_string(),
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
            worker.complete_debug_snapshot(0, r#"{"status":"ok"}"#.to_string());
        });

        let snapshot = bridge
            .request_debug_snapshot(Duration::from_millis(500))
            .unwrap();
        handle.join().unwrap();
        assert_eq!(snapshot, r#"{"status":"ok"}"#);
    }
}
