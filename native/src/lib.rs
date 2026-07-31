mod app;
#[cfg(test)]
mod command_queue_model;
mod commands;
pub mod css_style;
mod document;
mod error;
pub(crate) mod events;
pub(crate) mod html_report_webview;
pub(crate) mod icons;
pub(crate) mod image_widget;
pub(crate) mod layout;
pub(crate) mod overlays;
pub(crate) mod paint;
pub(crate) mod primitives;
pub(crate) mod resources;
mod runtime;
pub(crate) mod scatter;
pub(crate) mod style;
pub(crate) mod table;
pub(crate) mod text;
pub(crate) mod theme;
pub(crate) mod toast;
pub(crate) mod widget_capabilities;

pub(crate) const DEPTH_STENCIL_FORMAT: wgpu::TextureFormat =
    wgpu::TextureFormat::Depth24PlusStencil8;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use rfd::FileDialog;

#[pyfunction]
fn backend_info(py: Python<'_>) -> PyResult<Py<PyDict>> {
    let info = PyDict::new(py);
    info.set_item("name", "dragongui")?;
    info.set_item("native", true)?;
    info.set_item("renderer", "wgpu")?;
    info.set_item("status", "m8-w0-shipping-widgets")?;
    info.set_item("layout", "taffy")?;
    info.set_item("text", "glyphon")?;
    Ok(info.unbind())
}

#[pyfunction]
fn run_app(
    py: Python<'_>,
    document: Bound<'_, PyAny>,
    click_callbacks: Bound<'_, PyDict>,
    change_callbacks: Bound<'_, PyDict>,
) -> PyResult<Py<PyDict>> {
    if document.is_none() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "document cannot be None",
        ));
    }
    app::run_app_impl(py, document, click_callbacks, change_callbacks, None)
}

#[pyfunction]
fn run_app_with_handle(
    py: Python<'_>,
    document: Bound<'_, PyAny>,
    click_callbacks: Bound<'_, PyDict>,
    change_callbacks: Bound<'_, PyDict>,
    app_handle: Bound<'_, PyAny>,
) -> PyResult<Py<PyDict>> {
    if document.is_none() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "document cannot be None",
        ));
    }
    app::run_app_impl(
        py,
        document,
        click_callbacks,
        change_callbacks,
        Some(app_handle),
    )
}

#[pyfunction(signature = (title=None, filters=None))]
fn open_file_dialog(
    py: Python<'_>,
    title: Option<String>,
    filters: Option<Vec<(String, Vec<String>)>>,
) -> PyResult<Option<String>> {
    let path = py.allow_threads(|| {
        let dialog = apply_dialog_options(FileDialog::new(), title.as_deref(), filters.as_deref());
        dialog.pick_file()
    });
    Ok(path.map(|path| path.to_string_lossy().to_string()))
}

#[pyfunction(signature = (title=None, filters=None))]
fn open_files_dialog(
    py: Python<'_>,
    title: Option<String>,
    filters: Option<Vec<(String, Vec<String>)>>,
) -> PyResult<Option<Vec<String>>> {
    let paths = py.allow_threads(|| {
        let dialog = apply_dialog_options(FileDialog::new(), title.as_deref(), filters.as_deref());
        dialog.pick_files()
    });
    Ok(paths.map(|paths| {
        paths
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect()
    }))
}

#[pyfunction(signature = (title=None, filters=None))]
fn save_file_dialog(
    py: Python<'_>,
    title: Option<String>,
    filters: Option<Vec<(String, Vec<String>)>>,
) -> PyResult<Option<String>> {
    let path = py.allow_threads(|| {
        let dialog = apply_dialog_options(FileDialog::new(), title.as_deref(), filters.as_deref());
        dialog.save_file()
    });
    Ok(path.map(|path| path.to_string_lossy().to_string()))
}

#[pyfunction(signature = (title=None))]
fn pick_folder_dialog(py: Python<'_>, title: Option<String>) -> PyResult<Option<String>> {
    let path = py.allow_threads(|| {
        let mut dialog = FileDialog::new();
        if let Some(title) = title.as_deref().filter(|title| !title.is_empty()) {
            dialog = dialog.set_title(title);
        }
        dialog.pick_folder()
    });
    Ok(path.map(|path| path.to_string_lossy().to_string()))
}

fn apply_dialog_options(
    mut dialog: FileDialog,
    title: Option<&str>,
    filters: Option<&[(String, Vec<String>)]>,
) -> FileDialog {
    if let Some(title) = title.filter(|title| !title.is_empty()) {
        dialog = dialog.set_title(title);
    }
    if let Some(filters) = filters {
        for (name, extensions) in filters {
            let extensions: Vec<&str> = extensions
                .iter()
                .map(String::as_str)
                .filter(|ext| !ext.is_empty())
                .collect();
            if !name.is_empty() && !extensions.is_empty() {
                dialog = dialog.add_filter(name, &extensions);
            }
        }
    }
    dialog
}

#[pymodule]
fn _dragongui(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<commands::NativeCommandSender>()?;
    m.add_function(wrap_pyfunction!(backend_info, m)?)?;
    m.add_function(wrap_pyfunction!(run_app, m)?)?;
    m.add_function(wrap_pyfunction!(run_app_with_handle, m)?)?;
    m.add_function(wrap_pyfunction!(open_file_dialog, m)?)?;
    m.add_function(wrap_pyfunction!(open_files_dialog, m)?)?;
    m.add_function(wrap_pyfunction!(save_file_dialog, m)?)?;
    m.add_function(wrap_pyfunction!(pick_folder_dialog, m)?)?;
    Ok(())
}
