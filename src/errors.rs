use thiserror::Error;
use pyo3::{exceptions::PyValueError, PyErr};
use tf_demo_parser::ParseError;
use bitbuffer::BitError;
use serde_json::Number;
use serde_json_path::{ParseError as JsonPathParseError, AtMostOneError};
use polars::error::PolarsError;
use serde_arrow::Error as ArrowError;


pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("'{}' cannot be represented either as i- or f-64", .0)]
    InvalidNumber(Number),

    #[error("Wire format error: {0}")]
    WireFormat(#[from] ParseError),

    #[error("Buffering error: {0}")]
    Buffering(#[from] BitError),

    #[error("Python error: {0}")]
    Python(#[from] PyErr),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Arrow serialization error: {0}")]
    ArrowSerialization(#[from] ArrowError),

    #[error("JSON Path parse error: {0}")]
    PathParse(#[from] JsonPathParseError),

    #[error("JSON Path match error: {0}")]
    PathMatch(#[from] AtMostOneError),

    #[error("Polars error: {0}")]
    Polars(#[from] PolarsError),
}

impl From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}
