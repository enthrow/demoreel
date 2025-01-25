use serde_arrow::schema::{SchemaLike, TracingOptions};
use arrow::datatypes::FieldRef;
use polars::prelude::DataFrame;
use polars::series::Series;
use arrow::array::ArrayRef;
use pyo3::{
    types::{PyDict, PyList, PyDictMethods},
    IntoPy, PyObject, Python
};
use serde::{Serialize, Deserialize};
use serde_json_path::JsonPath;

use crate::errors::*;

pub fn json_to_py<'py>(py: Python<'py>, v: &serde_json::Value) -> Result<PyObject> {
    use serde_json::Value;
    match v {
        Value::Object(obj) => {
            let dict = PyDict::new_bound(py);
            for (k, v) in obj.into_iter() {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
            Ok(dict.into())
        }
        Value::Array(arr) => {
            let istrm: Result<Vec<_>> = arr.into_iter().map(|x| json_to_py(py, x)).collect();
            Ok(PyList::new_bound(py, istrm?).into())
        }
        Value::Null => Ok(py.None()),
        Value::Bool(p) => Ok(p.into_py(py)),
        Value::Number(n) => {
            if let Some(k) = n.as_i64() {
                Ok(k.into_py(py))
            } else if let Some(x) = n.as_f64() {
                Ok(x.into_py(py))
            } else {
                Err(Error::from(ErrorKind::InvalidNumber(n.clone())))
            }
        }
        Value::String(s) => Ok(s.into_py(py)),
    }
}

pub fn json_match<'v>(
    json_path: Option<&JsonPath>,
    payload: &'v serde_json::Value,
) -> Option<serde_json::Value> {
    if let Some(path) = json_path {
        let values = path
            .query(payload)
            .all()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        match values.len() {
            0 => None,
            1 => Some(values.into_iter().next().unwrap()),
            _ => Some(serde_json::Value::Array(values)),
        }
    } else {
        Some(payload.clone())
    }
}
pub fn to_polars<T: Serialize + for<'de> Deserialize<'de>>(
    values: &[T],
    config: Option<TracingOptions>,
) -> Result<Option<DataFrame>> {
    if values.is_empty() {
        return Ok(None);
    }
    let tracing_options = config.unwrap_or_else(TracingOptions::default);
    let fields = Vec::<FieldRef>::from_type::<T>(tracing_options)?;
    let arrays: Vec<ArrayRef> = serde_arrow::to_arrow(&fields, values)?;

    let mut series_vec = Vec::new();
    for (field, array) in fields.iter().zip(arrays) {
        let series_name = field.name().as_str().into(); // Get column name
        let series = Series::from_arrow_chunks(series_name, vec![array.into()])?; // Create Series from array
        series_vec.push(series);
    }

    let df: polars::prelude::DataFrame = DataFrame::new(series_vec)?;
    Ok(Some(df))
}
