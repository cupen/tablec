use tablec_core::core::project::project::Project;
use tablec_core::core::project::meta::Meta;
use tablec_core::core::table::table::Table;
use tablec_core::core::table::field::{Field, FieldType};
use tablec_core::core::table::row::Row;
use tablec_core::core::table::value::Value;
use tablec_core::export::{Format, Json, Msgpack};
use indexmap::IndexMap;

fn make_simple_project() -> Project {
    let tables = IndexMap::from([(
        "heroes".to_string(),
        Table {
            name: "heroes".to_string(),
            fields: vec![
                Field {
                    name: "id".to_string(),
                    t: FieldType::Int32,
                    desc: "hero id".to_string(),
                    constraint: None,
                    tags: vec![],
                },
                Field {
                    name: "name".to_string(),
                    t: FieldType::String,
                    desc: "hero name".to_string(),
                    constraint: None,
                    tags: vec![],
                },
                Field {
                    name: "level".to_string(),
                    t: FieldType::Int32,
                    desc: "hero level".to_string(),
                    constraint: None,
                    tags: vec![],
                },
            ],
            data: vec![
                Row::from_vec(vec![
                    ("id".to_string(), Value::Int32(1)),
                    ("name".to_string(), Value::String("Arthur".to_string())),
                    ("level".to_string(), Value::Int32(5)),
                ]),
                Row::from_vec(vec![
                    ("id".to_string(), Value::Int32(2)),
                    ("name".to_string(), Value::String("Lancelot".to_string())),
                    ("level".to_string(), Value::Int32(8)),
                ]),
            ],
            constraints: vec![],
        },
    )]);

    Project {
        name: "test_project".to_string(),
        meta: Meta {
            version: "1.0.0".to_string(),
            hash: 12345,
            build_at: 1700000000,
        },
        tables,
    }
}

// === JSON export ===

#[test]
fn test_json_export_pretty_with_fields() {
    let project = make_simple_project();
    let json = Json { pretty: true, include_fields: true };
    let result = json.to_vec(&project).unwrap();
    let s = String::from_utf8(result).unwrap();

    assert!(s.contains("test_project"));
    assert!(s.contains("heroes"));
    assert!(s.contains("Arthur"));
    assert!(s.contains("Lancelot"));
    assert!(s.contains("fields"));
    assert!(s.contains("hero id"));
    assert!(s.contains("hero name"));

    // Verify it's valid JSON
    let _parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
}

#[test]
fn test_json_export_pretty_without_fields() {
    let project = make_simple_project();
    let json = Json { pretty: true, include_fields: false };
    let result = json.to_vec(&project).unwrap();
    let s = String::from_utf8(result).unwrap();

    assert!(s.contains("test_project"));
    assert!(s.contains("Arthur"));
    assert!(!s.contains("fields"));
    assert!(!s.contains("hero id")); // desc should not appear

    let _parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
}

#[test]
fn test_json_export_compact() {
    let project = make_simple_project();
    let json = Json { pretty: false, include_fields: false };
    let result = json.to_vec(&project).unwrap();
    let s = String::from_utf8(result).unwrap();

    // Compact should not have pretty-print newlines
    assert!(s.contains("Arthur"));
    assert!(!s.contains("  ")); // no indentation
    let _parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
}

#[test]
fn test_json_export_has_meta() {
    let project = make_simple_project();
    let json = Json { pretty: true, include_fields: false };
    let result = json.to_vec(&project).unwrap();
    let s = String::from_utf8(result).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed["meta"]["version"], "1.0.0");
}

#[test]
fn test_json_export_empty_project() {
    let project = Project {
        name: "empty".to_string(),
        meta: Meta {
            version: "0.1.0".to_string(),
            hash: 0,
            build_at: 0,
        },
        tables: IndexMap::new(),
    };
    let json = Json { pretty: true, include_fields: false };
    let result = json.to_vec(&project).unwrap();
    let s = String::from_utf8(result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed["name"], "empty");
    assert!(parsed["tables"].as_array().unwrap().is_empty());
}

// === Msgpack export ===

#[test]
fn test_msgpack_export_roundtrip() {
    let project = make_simple_project();
    let msgpack = Msgpack;
    let bytes = msgpack.to_vec(&project).unwrap();
    assert!(!bytes.is_empty());

    // Roundtrip: deserialize back and verify
    let decoded: Project = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(decoded.name, "test_project");
    assert_eq!(decoded.meta.version, "1.0.0");
    assert_eq!(decoded.tables.len(), 1);

    let heroes = &decoded.tables["heroes"];
    assert_eq!(heroes.name, "heroes");
    assert_eq!(heroes.data.len(), 2);
    assert_eq!(heroes.fields.len(), 3);
}

#[test]
fn test_msgpack_export_empty_project() {
    let project = Project {
        name: "empty".to_string(),
        meta: Meta {
            version: "0.1.0".to_string(),
            hash: 0,
            build_at: 0,
        },
        tables: IndexMap::new(),
    };
    let msgpack = Msgpack;
    let bytes = msgpack.to_vec(&project).unwrap();
    let decoded: Project = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(decoded.name, "empty");
    assert_eq!(decoded.tables.len(), 0);
}

#[test]
fn test_msgpack_export_multi_table() {
    let tables = IndexMap::from([
        ("table_a".to_string(), Table {
            name: "table_a".to_string(),
            fields: vec![Field {
                name: "x".to_string(), t: FieldType::Int32,
                desc: "".to_string(), constraint: None, tags: vec![],
            }],
            data: vec![Row::from_vec(vec![("x".to_string(), Value::Int32(1))])],
            constraints: vec![],
        }),
        ("table_b".to_string(), Table {
            name: "table_b".to_string(),
            fields: vec![Field {
                name: "y".to_string(), t: FieldType::String,
                desc: "".to_string(), constraint: None, tags: vec![],
            }],
            data: vec![Row::from_vec(vec![("y".to_string(), Value::String("hello".to_string()))])],
            constraints: vec![],
        }),
    ]);

    let project = Project {
        name: "multi".to_string(),
        meta: Meta { version: "1.0.0".to_string(), hash: 0, build_at: 0 },
        tables,
    };

    let msgpack = Msgpack;
    let bytes = msgpack.to_vec(&project).unwrap();
    let decoded: Project = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(decoded.tables.len(), 2);
    assert!(decoded.tables.contains_key("table_a"));
    assert!(decoded.tables.contains_key("table_b"));
}

// === JSON value type coverage ===

#[test]
fn test_json_export_all_value_types() {
    let tables = IndexMap::from([(
        "all_types".to_string(),
        Table {
            name: "all_types".to_string(),
            fields: vec![],
            data: vec![
                Row::from_vec(vec![
                    ("int_val".to_string(), Value::Int32(42)),
                    ("uint_val".to_string(), Value::Uint32(100)),
                    ("float_val".to_string(), Value::Float32(3.14)),
                    ("string_val".to_string(), Value::String("hello".to_string())),
                    ("bool_val".to_string(), Value::Bool(true)),
                    ("null_val".to_string(), Value::Null),
                    ("array_val".to_string(), Value::Array(vec![
                        Value::Int32(1), Value::Int32(2),
                    ])),
                ]),
            ],
            constraints: vec![],
        },
    )]);

    let project = Project {
        name: "types_test".to_string(),
        meta: Meta { version: "1.0.0".to_string(), hash: 0, build_at: 0 },
        tables,
    };

    let json = Json { pretty: true, include_fields: false };
    let result = json.to_vec(&project).unwrap();
    let s = String::from_utf8(result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
    let row = &parsed["tables"][0]["data"][0];

    assert_eq!(row["int_val"], 42);
    assert_eq!(row["uint_val"], 100);
    // Float32 precision is coarser than 1e-10 around 3.14 — use a wider bound.
    assert!((row["float_val"].as_f64().unwrap() - 3.14).abs() < 1e-5);
    assert_eq!(row["string_val"], "hello");
    assert_eq!(row["bool_val"], true);
    assert!(row["null_val"].is_null());
    assert_eq!(row["array_val"].as_array().unwrap().len(), 2);
}

// === legacy to_string/to_vec compatibility ===
// (Legacy wrappers removed in c3; see brief step 8.)
