use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;

#[derive(thiserror::Error, Debug)]
pub enum DragonError {
    #[error("document parse error: {0}")]
    ParseError(String),
    #[error("GPU initialization failed: {0}")]
    GpuInit(String),
    #[error("render error: {0}")]
    Render(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}

impl From<DragonError> for PyErr {
    fn from(e: DragonError) -> Self {
        PyRuntimeError::new_err(e.to_string())
    }
}
