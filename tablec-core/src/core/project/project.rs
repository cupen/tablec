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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::{Field, FieldType};
    use crate::core::table::row::Row;
    use crate::core::table::value::Value;
    use std::path::PathBuf;
    use std::str::FromStr;

    fn empty_table(name: &str) -> Table {
        Table {
            name: name.to_string(),
            fields: vec![],
            data: vec![],
            constraints: vec![],
        }
    }

    #[test]
    fn from_tables_inserts_into_indexmap() {
        let tables = vec![empty_table("a"), empty_table("b")];
        let project = Project::from_tables("p".to_string(), tables);
        assert_eq!(project.name, "p");
        assert_eq!(project.tables.len(), 2);
        assert!(project.tables.contains_key("a"));
        assert!(project.tables.contains_key("b"));
        // IndexMap preserves insertion order.
        let keys: Vec<&String> = project.tables.keys().collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn from_tables_uses_meta_default() {
        let project = Project::from_tables("p".to_string(), vec![]);
        // from_tables does NOT call calculate_hash; hash stays at the default zero buffer.
        assert_eq!(project.meta.hash, [0u8; 32]);
        assert!(project.meta.source.is_empty());
    }

    #[test]
    fn from_excel_nonexistent_file_returns_err() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = dir.path().join("does_not_exist.xlsx");
        let result = Project::from_excel("p".to_string(), bogus.to_str().unwrap());
        assert!(result.is_err(), "expected error for missing file");
    }

    #[test]
    fn from_tables_with_source_stamps_meta_and_seeds_hash() {
        let tables = vec![empty_table("a")];
        let source = vec![PathBuf::from("/tmp/a.xlsx")];
        let project = Project::from_tables_with_source("p".to_string(), tables, source);
        assert_eq!(project.meta.source.len(), 1);
        assert_eq!(project.meta.source[0], PathBuf::from("/tmp/a.xlsx"));
        // from_tables_with_source calls calculate_hash; hash must not be all-zero.
        assert_ne!(project.meta.hash, [0u8; 32]);
    }

    #[test]
    fn calculate_hash_is_deterministic_for_same_content() {
        let mk = || {
            let table = Table {
                name: "t".to_string(),
                fields: vec![Field {
                    name: "id".to_string(),
                    t: FieldType::Int32,
                    desc: String::new(),
                    constraint: None,
                    tags: vec![],
                }],
                data: vec![
                    Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
                    Row::from_vec(vec![("id".to_string(), Value::Int32(2))]),
                ],
                constraints: vec![],
            };
            let mut p = Project::from_tables("p".to_string(), vec![table]);
            p.calculate_hash();
            p.meta.hash
        };
        assert_eq!(mk(), mk(), "hash must be deterministic across calls");
    }

    #[test]
    fn calculate_hash_changes_when_table_data_changes() {
        let mk = |id_value: i32| {
            let table = Table {
                name: "t".to_string(),
                fields: vec![Field {
                    name: "id".to_string(),
                    t: FieldType::Int32,
                    desc: String::new(),
                    constraint: None,
                    tags: vec![],
                }],
                data: vec![Row::from_vec(vec![(
                    "id".to_string(),
                    Value::Int32(id_value),
                )])],
                constraints: vec![],
            };
            let mut p = Project::from_tables("p".to_string(), vec![table]);
            p.calculate_hash();
            p.meta.hash
        };
        assert_ne!(mk(1), mk(2), "different data must produce different hash");
    }

    #[test]
    fn calculate_hash_independent_of_field_declaration_order() {
        let mk = |fields: Vec<Field>| {
            let table = Table {
                name: "t".to_string(),
                fields,
                data: vec![Row::from_vec(vec![
                    ("a".to_string(), Value::Int32(1)),
                    ("b".to_string(), Value::Int32(2)),
                ])],
                constraints: vec![],
            };
            let mut p = Project::from_tables("p".to_string(), vec![table]);
            p.calculate_hash();
            p.meta.hash
        };
        let field_a = Field {
            name: "a".to_string(),
            t: FieldType::Int32,
            desc: String::new(),
            constraint: None,
            tags: vec![],
        };
        let field_b = Field {
            name: "b".to_string(),
            t: FieldType::Int32,
            desc: String::new(),
            constraint: None,
            tags: vec![],
        };
        let ab = mk(vec![field_a.clone(), field_b.clone()]);
        let ba = mk(vec![field_b, field_a]);
        assert_eq!(
            ab, ba,
            "hash must be order-independent in field declaration"
        );
    }

    #[test]
    fn validate_all_empty_project_returns_ok() {
        let project = Project::from_tables("p".to_string(), vec![]);
        let result = project.validate_all();
        assert!(result.is_ok(), "empty project must validate cleanly");
    }

    #[test]
    fn validate_all_collects_constraint_violations() {
        use crate::core::table::constraint::Constraint;
        let table = Table {
            name: "t".to_string(),
            fields: vec![Field {
                name: "id".to_string(),
                t: FieldType::Int32,
                desc: String::new(),
                constraint: Constraint::from_str("@unique").ok(),
                tags: vec![],
            }],
            // Two rows with the same id -> @unique violation.
            data: vec![
                Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
                Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
            ],
            constraints: vec![],
        };
        let project = Project::from_tables("p".to_string(), vec![table]);
        let errs = project.validate_all().unwrap_err();
        assert!(
            !errs.is_empty(),
            "validate_all must surface per-table constraint violations"
        );
    }

    #[test]
    fn export_writes_to_output_path() {
        let project = Project::from_tables("p".to_string(), vec![empty_table("a")]);
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.json");
        let json = crate::export::Json {
            pretty: false,
            include_fields: false,
        };
        project.export(&json, out.to_str().unwrap()).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert!(!content.is_empty());
        assert!(content.contains("a"));
    }
}
