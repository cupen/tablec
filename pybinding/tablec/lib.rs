use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::Bound;
use ::tablec as tablec_mod;

#[pyclass]
struct Tablec;

#[pymethods]
impl Tablec {
    #[staticmethod]
    #[pyo3(signature = (input, output, format, include_fields = false))]
    fn build(input: &str, output: &str, format: &str, include_fields: bool) -> PyResult<()> {
        let c = tablec_mod::cmd::build::BuildCommand {
            input: input.to_string(),
            output: output.to_string(),
            format: format.to_string(),
            include_fields,
        };
        c.run().map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(())
    }

    #[staticmethod]
    fn check(path: &str) -> PyResult<()> {
        let c = tablec_mod::cmd::check::CheckCommand {
            verbose: false,
            path: Some(path.into()),
        };
        c.run().map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(())
    }
}

#[pymodule]
fn _native(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Tablec>()?;
    Ok(())
}