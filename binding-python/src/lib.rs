use std::sync::Arc;

use pyo3::Bound;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use tablec_core::core::project::meta::{Meta, ToolVersion};
use tablec_core::core::project::project::Project;
use tablec_core::core::schema::{SchemaParser, SchemaParserRegistry};
use tablec_core::core::table::table::Table;
use tablec_core::export::{Format, Json, Msgpack};

fn read_excel_with_parser(input: &str, parser: Arc<dyn SchemaParser>) -> PyResult<Vec<Table>> {
    tablec_core::core::table::table::read_excel_with(input, parser.as_ref()).map_err(|errs| {
        let msg = errs
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        pyo3::exceptions::PyValueError::new_err(msg)
    })
}

fn resolve_parser(parser: Option<&str>) -> PyResult<Arc<dyn SchemaParser>> {
    let parser_name = parser.unwrap_or("standard");
    SchemaParserRegistry::with_standard()
        .get(parser_name)
        .ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "parser '{}' not registered",
                parser_name
            ))
        })
}

#[pyfunction]
#[pyo3(signature = (input, output=None, format=None, parser=None))]
fn build(
    input: &str,
    output: Option<&str>,
    format: Option<&str>,
    parser: Option<&str>,
) -> PyResult<()> {
    let output = output
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("output is required".to_string()))?;
    let format = format
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("format is required".to_string()))?;
    let parser_arc = resolve_parser(parser)?;
    let tables = read_excel_with_parser(input, parser_arc)?;

    let project = Project {
        name: std::path::Path::new(input)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string(),
        meta: Meta {
            version: "0.0.0".to_string(),
            hash: [0u8; 32],
            build_at: 0,
            source: vec![],
            tool: ToolVersion::default(),
        },
        tables: tables.into_iter().map(|t| (t.name.clone(), t)).collect(),
    };

    let bytes: Vec<u8> = match format {
        "json" => Json {
            pretty: false,
            include_fields: false,
        }
        .to_vec(&project)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        "json-pretty" => Json {
            pretty: true,
            include_fields: false,
        }
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
#[pyo3(signature = (input, parser=None))]
fn check(input: &str, parser: Option<&str>) -> PyResult<()> {
    let parser_arc = resolve_parser(parser)?;
    let tables = read_excel_with_parser(input, parser_arc)?;

    for table in &tables {
        if let Err(errs) = table.validate_constraints() {
            let msg = errs
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("\n");
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
