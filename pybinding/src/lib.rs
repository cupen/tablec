use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::Bound;
use ::tablec as tablec_mod; 

#[pyfunction]
fn build(input: &str, output: &str, format: &str) -> PyResult<()> {
    let c = tablec_mod::cmd::build::BuildCommand {
        input: input.to_string(),
        output: output.to_string(),
        format: format.to_string(),
    };
    c.run().map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
}

#[pymodule]
fn tablec(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(build, m)?)?;
    Ok(())
}
