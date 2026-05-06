mod app;
mod commands;
pub mod css_style;
mod document;
mod error;
pub(crate) mod events;
pub(crate) mod html_report_webview;
pub(crate) mod image_widget;
pub(crate) mod layout;
pub(crate) mod overlays;
pub(crate) mod primitives;
pub(crate) mod resources;
mod runtime;
mod runtime_profile;
pub(crate) mod scatter;
pub(crate) mod style;
pub(crate) mod table;
pub(crate) mod text;
pub(crate) mod theme;
pub(crate) mod toast;

pub(crate) const DEPTH_STENCIL_FORMAT: wgpu::TextureFormat =
    wgpu::TextureFormat::Depth24PlusStencil8;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use rfd::FileDialog;

#[pyfunction]
fn backend_info(py: Python<'_>) -> PyResult<Py<PyDict>> {
    let profile = runtime_profile::RuntimeProfileSelection::current();
    let info = PyDict::new(py);
    info.set_item("name", "dragongui")?;
    info.set_item("native", true)?;
    info.set_item("renderer", "wgpu")?;
    info.set_item("status", "m8-w0-shipping-widgets")?;
    info.set_item("layout", "taffy")?;
    info.set_item("text", "glyphon")?;

    let platform = PyDict::new(py);
    platform.set_item("os", runtime_profile::target_os())?;
    platform.set_item("arch", runtime_profile::target_arch())?;
    platform.set_item("profile", profile.profile.as_str())?;
    platform.set_item("profile_requested", profile.requested.as_str())?;
    platform.set_item("profile_source", profile.source)?;
    platform.set_item("pi_feature", profile.pi_feature)?;
    platform.set_item("auto_pi_target", profile.auto_pi_target)?;
    platform.set_item("scatter_max_points", profile.scatter_max_points())?;
    platform.set_item("scatter_lod_threshold", profile.scatter_lod_threshold())?;
    platform.set_item("line_plot_max_points", profile.line_plot_max_points())?;
    platform.set_item("table_page_size", profile.table_page_size())?;
    platform.set_item("table_sample_rows", profile.table_sample_rows())?;
    platform.set_item(
        "table_column_buffer_rows",
        profile.table_column_buffer_rows(),
    )?;
    platform.set_item(
        "wgpu_backend_override",
        std::env::var("DRAGONGUI_WGPU_BACKEND").ok(),
    )?;
    info.set_item("platform", platform)?;

    let features = PyDict::new(py);
    features.set_item("pi", cfg!(feature = "pi"))?;
    features.set_item("gpu", cfg!(feature = "gpu"))?;
    features.set_item("webview", runtime_profile::embedded_webview_available())?;
    info.set_item("features", features)?;
    info.set_item(
        "webview_available",
        runtime_profile::embedded_webview_available(),
    )?;
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
