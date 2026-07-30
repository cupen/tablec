use super::super::config::Config;
use super::meta::Meta;
use crate::core::table::field::Field;
use crate::core::table::table::Table;
use crate::export::Format;
use blake3::Hasher;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::error::Error;

fn read_excel_with_box(path: &str) -> Result<Vec<Table>, Box<dyn std::error::Error>> {
    crate::core::table::table::read_excel(path).map_err(|errs| {
        let msg = errs
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        Box::<dyn std::error::Error>::from(msg)
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub meta: Meta,
    pub tables: IndexMap<String, Table>,
}

impl Project {
    pub fn from_tables(name: String, tables: Vec<Table>) -> Self {
        let mut table_map = IndexMap::new();
        for table in tables {
            table_map.insert(table.name.clone(), table);
        }

        Project {
            name,
            meta: Meta::default(),
            tables: table_map,
        }
    }

    pub fn from_excel(name: String, path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let tables = read_excel_with_box(path)?;
        let source = vec![std::path::PathBuf::from(path)];
        Ok(Self::from_tables_with_source(name, tables, source))
    }

    pub fn from_config(
        config: &Config,
        input_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let tables = read_excel_with_box(input_path)?;
        let name = config.project.name.clone();
        let source = vec![std::path::PathBuf::from(input_path)];
        Ok(Self::from_tables_with_source(name, tables, source))
    }

    /// Same as `from_tables` but stamps `source` (input file paths) into
    /// `Meta` and seeds `Meta.hash` via `calculate_hash`. Use this when the
    /// final artifact's hash should be observable to consumers.
    pub fn from_tables_with_source(
        name: String,
        tables: Vec<Table>,
        source: Vec<std::path::PathBuf>,
    ) -> Self {
        let mut project = Self::from_tables(name, tables);
        project.meta.source = source;
        project.calculate_hash();
        project
    }

    pub fn calculate_hash(&mut self) {
        let mut hasher: Hasher = blake3::Hasher::new_derive_key("tablec.project.v1");
        hasher.update(self.name.as_bytes());

        // Hash sheets in a stable order regardless of IndexMap iteration order.
        let mut sheets: Vec<(&String, &Table)> = self.tables.iter().collect();
        sheets.sort_by(|a, b| a.0.cmp(b.0));

        for (sheet_name, table) in sheets {
            hasher.update(sheet_name.as_bytes());

            // Schema (canonical = JSON with sorted field names).
            let fields_canon = serde_json::to_vec(&canonical_fields(&table.schema.fields))
                .expect("fields always serializable");
            hasher.update(&fields_canon);

            // Data rows, row-order sensitive (any reorder/delete -> byte stream change -> hash change).
            for row in &table.data {
                let row_canon = serde_json::to_vec(&row.fields).expect("row always serializable");
                hasher.update(&row_canon);
            }
        }

        self.meta.hash = *hasher.finalize().as_bytes();
    }

    pub fn validate_all(&self) -> Result<(), Vec<crate::core::diagnostic::Diagnostic>> {
        let mut all_errors = Vec::new();

        for (_table_name, table) in &self.tables {
            if let Err(errors) = table.validate_constraints() {
                for d in errors {
                    all_errors.push(d);
                }
            }
        }

        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }

    pub fn export<F: Format>(&self, format: &F, output: &str) -> Result<(), Box<dyn Error>> {
        format.export(self, output)
    }
}

/// Render a `Field` set as a `serde_json::Value` object keyed by field name, with
/// field names sorted alphabetically. This canonicalization makes the hash
/// independent of the field declaration order in the schema, while still
/// distinguishing schemas with different field names or types.
fn canonical_fields(fields: &[Field]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let mut names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    names.sort();
    for n in names {
        let f = fields.iter().find(|x| x.name == n).unwrap();
        map.insert(n.to_string(), serde_json::json!(format!("{:?}", f.t)));
    }
    serde_json::Value::Object(map)
}
