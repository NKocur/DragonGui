use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use std::collections::HashMap;

use crate::commands::{CommandBridge, NativeCommandSender};
use crate::css_style::apply_stylesheets_to_tree;
use crate::document::{self, StartupDoc, WidgetNode};
use crate::error::DragonError;
use crate::events::ChangeValue;
use crate::runtime::{run_event_loop, AppSpec};
use crate::theme::Theme;

pub fn run_app_impl(
    py: Python<'_>,
    document: Bound<'_, PyAny>,
    click_callbacks: Bound<'_, PyDict>,
    change_callbacks: Bound<'_, PyDict>,
    app_handle: Option<Bound<'_, PyAny>>,
) -> PyResult<Py<PyDict>> {
    // Serialize the Python dict to JSON while holding the GIL.
    let json_str: String = py
        .import("json")?
        .call_method1("dumps", (&document,))?
        .extract()?;

    let raw: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| DragonError::ParseError(e.to_string()))?;

    let python_theme: Option<Theme> = document::parse_theme_from_doc(&raw);
    let loading_screen = document::parse_loading_screen_from_doc(&raw);
    let effective_theme = python_theme.clone().unwrap_or_else(Theme::dark);
    let icon_theme = crate::icons::IconThemeRegistry::from_value(raw.get("icon_theme"))
        .map_err(DragonError::ParseError)?;
    let mut stylesheets = document::parse_stylesheets_from_doc(&raw);
    stylesheets.install_framework_defaults(&effective_theme);
    let mut widget_tree: Option<WidgetNode> = document::parse_widget_tree(&raw);
    if let Some(tree) = &mut widget_tree {
        icon_theme
            .apply_to_tree(tree)
            .map_err(DragonError::ParseError)?;
        apply_stylesheets_to_tree(tree, &mut stylesheets);
    }

    let doc: StartupDoc =
        serde_json::from_value(raw).map_err(|e| DragonError::ParseError(e.to_string()))?;

    // Wrap Python callables into thread-safe closures.  The closures capture
    // `Py<PyAny>` (which is Send) and re-acquire the GIL when called from
    // inside `py.detach`.
    let click_cbs: HashMap<String, Box<dyn Fn() + Send>> = click_callbacks
        .iter()
        .filter_map(|(k, v)| {
            let id = k.extract::<String>().ok()?;
            let cb: Py<PyAny> = v.unbind();
            let f: Box<dyn Fn() + Send> = Box::new(move || {
                Python::attach(|py| {
                    if let Err(err) = cb.call0(py) {
                        err.print(py);
                    }
                });
            });
            Some((id, f))
        })
        .collect();

    let change_cbs: HashMap<String, Box<dyn Fn(ChangeValue) + Send>> = change_callbacks
        .iter()
        .filter_map(|(k, v)| {
            let id = k.extract::<String>().ok()?;
            let cb: Py<PyAny> = v.unbind();
            let f: Box<dyn Fn(ChangeValue) + Send> = Box::new(move |val: ChangeValue| {
                Python::attach(|py| {
                    let result = match val {
                        ChangeValue::Bool(b) => cb.call1(py, (b,)),
                        ChangeValue::Float(f) => cb.call1(py, (f,)),
                        ChangeValue::Text(s) => cb.call1(py, (s,)),
                    };
                    if let Err(err) = result {
                        err.print(py);
                    }
                });
            });
            Some((id, f))
        })
        .collect();

    let (command_bridge, python_runtime) = if let Some(handle) = app_handle {
        let bridge = std::sync::Arc::new(CommandBridge::new());
        let sender = Py::new(py, NativeCommandSender::new(std::sync::Arc::clone(&bridge)))?;
        handle.call_method1("_bind_native_sender", (sender,))?;
        (Some(bridge), Some(handle.unbind()))
    } else {
        (None, None)
    };

    let decorations = doc.window.props.decorations.trim().to_ascii_lowercase();
    if decorations != "native" && decorations != "client" {
        return Err(DragonError::ParseError(format!(
            "window decorations must be 'native' or 'client', got {:?}",
            doc.window.props.decorations
        ))
        .into());
    }

    let spec = AppSpec {
        title: doc.window.props.title,
        width: doc.window.props.width,
        height: doc.window.props.height,
        client_decorations: decorations == "client",
        widget_tree,
        theme: python_theme,
        icon_theme,
        stylesheets,
        click_callbacks: click_cbs,
        change_callbacks: change_cbs,
        command_bridge,
        python_runtime,
        loading_screen,
    };

    let run_result = py.detach(|| run_event_loop(spec))?;

    let result = PyDict::new(py);
    result.set_item("status", "ok")?;
    result.set_item("renderer", "wgpu")?;
    result.set_item("upload_ms", run_result.upload_ms)?;
    result.set_item("frame_ms", run_result.frame_ms)?;
    let debug_snapshot = py
        .import("json")?
        .call_method1("loads", (&run_result.debug_snapshot,))?;
    result.set_item("debug_snapshot", debug_snapshot)?;
    Ok(result.unbind())
}
