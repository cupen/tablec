use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::Bound;
use tablec_core::core::table::table::read_excel;
use tablec_core::export;

#[pyfunction]
fn build(input: &str, output: &str, format: &str) -> PyResult<()> {
    let tables = read_excel(input).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    match format {
        "json" => {
            let json_data = export::json::to_string(&tables, false).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            std::fs::write(output, json_data).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        }
        "msgpack" => {
            let msgpack_data = export::msgpack::to_vec(&tables).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
            std::fs::write(output, msgpack_data).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        }
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!("Unsupported format '{}'", format)));
        }
    }

    Ok(())
}

#[pyfunction]
fn check(input: &str) -> PyResult<()> {
    let tables = read_excel(input).map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    for table in &tables {
        table.validate_constraints()
            .map_err(|errors| {
                pyo3::exceptions::PyValueError::new_err(
                    format!("Validation failed for table '{}': {}", table.name, errors.join("; "))
                )
            })?;
    }

    Ok(())
}


#[pymodule]
fn _native(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(build, m)?)?;
    m.add_function(wrap_pyfunction!(check, m)?)?;
    Ok(())
}
