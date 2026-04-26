mod app;
mod commands;
mod document;
mod error;
pub(crate) mod events;
pub(crate) mod layout;
pub(crate) mod primitives;
pub(crate) mod resources;
mod runtime;
pub(crate) mod scatter;
pub(crate) mod style;
pub(crate) mod table;
pub(crate) mod text;
pub(crate) mod theme;

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};

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

#[pymodule]
fn _dragongui(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<commands::NativeCommandSender>()?;
    m.add_function(wrap_pyfunction!(backend_info, m)?)?;
    m.add_function(wrap_pyfunction!(run_app, m)?)?;
    m.add_function(wrap_pyfunction!(run_app_with_handle, m)?)?;
    Ok(())
}
