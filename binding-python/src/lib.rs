use pyo3::prelude::*;
use pyo3::types::PyModule;
use pyo3::Bound;
use tablec_core::core::project::meta::{Meta, ToolVersion};
use tablec_core::core::project::project::Project;
use tablec_core::core::table::table::Table;
use tablec_core::export::{Format, Json, Msgpack};

fn read_excel_or_pyerr(input: &str) -> PyResult<Vec<Table>> {
    tablec_core::core::table::table::read_excel(input).map_err(|errs| {
        let msg = errs.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
        pyo3::exceptions::PyValueError::new_err(msg)
    })
}

#[pyfunction]
fn build(input: &str, output: &str, format: &str) -> PyResult<()> {
    let tables = read_excel_or_pyerr(input)?;

    let project = Project {
        name: std::path::Path::new(input)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string(),
        meta: Meta { version: "0.0.0".to_string(), hash: [0u8; 32], build_at: 0, source: vec![], tool: ToolVersion::default() },
        tables: tables.into_iter().map(|t| (t.name.clone(), t)).collect(),
    };

    let bytes: Vec<u8> = match format {
        "json" => Json { pretty: false, include_fields: false }
            .to_vec(&project)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        "json-pretty" => Json { pretty: true, include_fields: false }
            .to_vec(&project)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        "msgpack" => Msgpack
            .to_vec(&project)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        other => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unsupported format '{}'. Use one of: json, json-pretty, msgpack.",
                other
            )));
        }
    };
    std::fs::write(output, bytes)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

    Ok(())
}

#[pyfunction]
fn check(input: &str) -> PyResult<()> {
    let tables = read_excel_or_pyerr(input)?;

    for table in &tables {
        if let Err(errs) = table.validate_constraints() {
            let msg = errs.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
            return Err(pyo3::exceptions::PyValueError::new_err(msg));
        }
    }

    Ok(())
}


#[pymodule]
fn _native(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(build, m)?)?;
    m.add_function(wrap_pyfunction!(check, m)?)?;
    Ok(())
}
