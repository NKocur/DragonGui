use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;

pyo3::create_exception!(_dragongui, ScatterCapacityError, PyRuntimeError);

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
    #[error("{0}")]
    ScatterCapacity(String),
}

impl From<DragonError> for PyErr {
    fn from(e: DragonError) -> Self {
        match e {
            DragonError::ScatterCapacity(message) => ScatterCapacityError::new_err(message),
            other => PyRuntimeError::new_err(other.to_string()),
        }
    }
}
