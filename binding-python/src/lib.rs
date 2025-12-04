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
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!("Unsupported format '{}'", format)));
        }
    }

    Ok(())
}


#[pymodule]
fn tablec(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(build, m)?)?;
    Ok(())
}
