use std::error::Error;
use crate::core::project::project::Project;
use crate::export::Format;
use serde_json::json;

/// JSON export format
pub struct Json {
    pub pretty: bool,
    pub include_fields: bool,
}

impl Format for Json {
    fn export(&self, project: &Project, output: &str) -> Result<(), Box<dyn Error>> {
        let data = self.to_vec(project)?;
        if let Some(parent) = std::path::Path::new(output).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output, data)?;
        println!("Exported data to {}", output);
        Ok(())
    }

    fn to_vec(&self, project: &Project) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut tables_json = Vec::new();

        for (table_name, table) in &project.tables {
            let mut table_json = json!({
                "name": table_name,
                "data": table.data
            });

            if self.include_fields {
                table_json["fields"] = json!(table.fields);
            }

            tables_json.push(table_json);
        }

        let project_json = json!({
            "name": project.name,
            "meta": project.meta,
            "tables": tables_json
        });

        let data = if self.pretty {
            serde_json::to_string_pretty(&project_json)?
        } else {
            serde_json::to_string(&project_json)?
        };

        Ok(data.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::{Field, FieldType};
    use crate::core::table::row::Row;
    use crate::core::table::table::Table;
    use crate::core::table::value::Value;

    fn mk_field(name: &str, t: FieldType) -> Field {
        Field {
            name: name.to_string(),
            t,
            desc: String::new(),
            constraint: None,
            tags: Vec::new(),
        }
    }

    /// Spec T1: row-object keys must follow the sheet column order.
    /// Currently FAILs on the unmodified Cargo.toml because `serde_json`'s
    /// default `Map` is a `BTreeMap` that alphabetises keys.
    ///
    /// We re-parse the JSON output and inspect the row object's iteration
    /// order via a JSON pointer. That avoids substring collisions from
    /// the project / table "name" field elsewhere in the document.
    /// (After the `serde_json/preserve_order` fix, the parser's Map
    /// also becomes IndexMap-backed, so iteration matches the byte
    /// stream order — i.e. sheet column order — directly.)
    #[test]
    fn row_object_keys_in_sheet_column_order() {
        let row = Row::from_vec(vec![
            ("id".to_string(),   Value::Int32(1)),
            ("name".to_string(), Value::String("Alice".to_string())),
            ("age".to_string(),  Value::Int32(30)),
        ]);
        let table = Table {
            name: "users".to_string(),
            fields: vec![
                mk_field("id",   FieldType::Int32),
                mk_field("name", FieldType::String),
                mk_field("age",  FieldType::Int32),
            ],
            data: vec![row],
            constraints: vec![],
        };
        let project = Project::from_tables("test".to_string(), vec![table]);

        for pretty in [false, true] {
            let bytes = Json { pretty, include_fields: false }
                .to_vec(&project)
                .expect("json export");
            let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json");

            let row_value = v
                .pointer("/tables/0/data/0")
                .unwrap_or_else(|| panic!("missing /tables/0/data/0 (pretty={})", pretty));
            let obj = row_value
                .as_object()
                .unwrap_or_else(|| panic!("row is not an object (pretty={})", pretty));
            let keys: Vec<String> = obj.keys().cloned().collect();

            assert_eq!(
                keys,
                vec!["id".to_string(), "name".to_string(), "age".to_string()],
                "row keys should follow sheet column order (pretty={pretty})\n\
                 got: {keys:?}",
                pretty = pretty,
                keys = keys,
            );
        }
    }

    /// Spec T2: a row whose parse fails on a column yields a row object that
    /// simply omits that key (mirroring `read_excel`'s `Err` branch), while
    /// remaining keys keep their column order.
    #[test]
    fn row_object_keys_skip_parsed_value_error() {
        let row = Row::from_vec(vec![
            ("id".to_string(),   Value::Int32(7)),
            ("name".to_string(), Value::String("Bob".to_string())),
            // ("age" intentionally not inserted)
        ]);
        let table = Table {
            name: "users".to_string(),
            fields: vec![
                mk_field("id",   FieldType::Int32),
                mk_field("name", FieldType::String),
                mk_field("age",  FieldType::Int32),
            ],
            data: vec![row],
            constraints: vec![],
        };
        let project = Project::from_tables("test".to_string(), vec![table]);

        let bytes = Json { pretty: false, include_fields: false }
            .to_vec(&project)
            .expect("json export");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse");
        let row_value = v.pointer("/tables/0/data/0").expect("row present");
        let obj = row_value.as_object().expect("row is object");
        let keys: Vec<String> = obj.keys().cloned().collect();

        assert_eq!(
            keys,
            vec!["id".to_string(), "name".to_string()],
            "row should drop 'age' and keep id<name order, got {keys:?}",
            keys = keys,
        );
    }

    /// Spec T3: a column whose header starts with `#` is filtered out by
    /// `read_excel`, so the resulting row never references it and the row
    /// object's keys come from the surviving sheet columns.
    #[test]
    fn row_object_keys_drop_commented_columns() {
        let row = Row::from_vec(vec![
            ("id".to_string(),  Value::Int32(42)),
            ("age".to_string(), Value::Int32(99)),
        ]);
        let table = Table {
            name: "users".to_string(),
            // No Field for "secret" — it was filtered during read_excel.
            fields: vec![
                mk_field("id",  FieldType::Int32),
                mk_field("age", FieldType::Int32),
            ],
            data: vec![row],
            constraints: vec![],
        };
        let project = Project::from_tables("test".to_string(), vec![table]);

        let bytes = Json { pretty: false, include_fields: false }
            .to_vec(&project)
            .expect("json export");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse");
        let row_value = v.pointer("/tables/0/data/0").expect("row present");
        let obj = row_value.as_object().expect("row is object");
        let keys: Vec<String> = obj.keys().cloned().collect();

        assert_eq!(
            keys,
            vec!["id".to_string(), "age".to_string()],
            "row should drop commented column and keep sheet order, got {keys:?}",
            keys = keys,
        );
    }

    /// Spec T4: turning on `include_fields` must not change row key order.
    /// The `fields` array itself is a `Vec<Field>` and already preserves
    /// declared order through serde's array serialization; we verify both.
    #[test]
    fn include_fields_does_not_break_row_order() {
        let row = Row::from_vec(vec![
            ("id".to_string(),   Value::Int32(11)),
            ("name".to_string(), Value::String("Carol".to_string())),
            ("age".to_string(),  Value::Int32(45)),
        ]);
        let table = Table {
            name: "users".to_string(),
            fields: vec![
                mk_field("id",   FieldType::Int32),
                mk_field("name", FieldType::String),
                mk_field("age",  FieldType::Int32),
            ],
            data: vec![row],
            constraints: vec![],
        };
        let project = Project::from_tables("test".to_string(), vec![table]);

        let bytes = Json { pretty: false, include_fields: true }
            .to_vec(&project)
            .expect("json export");
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse");

        // Row objects retain sheet order.
        let row_value = v.pointer("/tables/0/data/0").expect("row present");
        let obj = row_value.as_object().expect("row is object");
        let keys: Vec<String> = obj.keys().cloned().collect();
        assert_eq!(
            keys,
            vec!["id".to_string(), "name".to_string(), "age".to_string()],
            "row keys should follow sheet column order (include_fields=true), got {keys:?}",
            keys = keys,
        );

        // The `fields` array preserves declaration order.
        let fields_value = v.pointer("/tables/0/fields").expect("fields present");
        let fields = fields_value.as_array().expect("fields is array");
        let field_names: Vec<&str> = fields
            .iter()
            .map(|f| f.get("name").and_then(|n| n.as_str()).expect("name str"))
            .collect();
        assert_eq!(
            field_names,
            vec!["id", "name", "age"],
            "fields array order should match declaration order"
        );
    }

    /// Spec T5: compact and pretty output must yield the same iteration
    /// order of row keys. Whitespace differences between the two are
    /// irrelevant; key ordering is the contract.
    #[test]
    fn pretty_and_compact_have_same_key_order() {
        let row = Row::from_vec(vec![
            ("id".to_string(),   Value::Int32(3)),
            ("name".to_string(), Value::String("Dave".to_string())),
            ("age".to_string(),  Value::Int32(50)),
        ]);
        let table = Table {
            name: "users".to_string(),
            fields: vec![
                mk_field("id",   FieldType::Int32),
                mk_field("name", FieldType::String),
                mk_field("age",  FieldType::Int32),
            ],
            data: vec![row],
            constraints: vec![],
        };
        let project = Project::from_tables("test".to_string(), vec![table]);

        let compact: serde_json::Value = serde_json::from_slice(
            &Json { pretty: false, include_fields: false }.to_vec(&project).expect("compact"),
        ).expect("parse compact");
        let pretty: serde_json::Value = serde_json::from_slice(
            &Json { pretty: true, include_fields: false }.to_vec(&project).expect("pretty"),
        ).expect("parse pretty");

        let compact_keys: Vec<String> = compact
            .pointer("/tables/0/data/0").and_then(|r| r.as_object())
            .expect("compact row").keys().cloned().collect();
        let pretty_keys: Vec<String> = pretty
            .pointer("/tables/0/data/0").and_then(|r| r.as_object())
            .expect("pretty row").keys().cloned().collect();

        assert_eq!(
            compact_keys, pretty_keys,
            "compact and pretty must produce the same row key order, got compact={compact_keys:?} pretty={pretty_keys:?}",
            compact_keys = compact_keys,
            pretty_keys = pretty_keys,
        );
        assert_eq!(
            compact_keys,
            vec!["id".to_string(), "name".to_string(), "age".to_string()],
            "row keys should be in sheet column order, got {compact_keys:?}",
            compact_keys = compact_keys,
        );
    }
}
