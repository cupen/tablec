use indexmap::IndexMap;
use std::collections::HashMap;
use std::str::FromStr;
use tablec_core::core::project::meta::{Meta, ToolVersion};
use tablec_core::core::project::project::Project;
use tablec_core::core::schema::Schema;
use tablec_core::core::table::constraint::Constraint;
use tablec_core::core::table::field::{Field, FieldType};
use tablec_core::core::table::row::Row;
use tablec_core::core::table::table::{Table, read_excel};
use tablec_core::core::table::value::Value;
use tablec_core::export::{Format, Json, Msgpack};

// === Full pipeline: build → validate → export ===

fn build_multi_table_project() -> Project {
    let tables = IndexMap::from([
        (
            "items".to_string(),
            Table {
                name: "items".to_string(),
                schema: Schema::from_parts(
                    vec![
                        Field {
                            name: "id".to_string(),
                            t: FieldType::Int32,
                            desc: "item id".to_string(),
                            constraint: Some(Constraint::from_str("@unique").unwrap()),
                            tags: vec!["pk".to_string()],
                        },
                        Field {
                            name: "name".to_string(),
                            t: FieldType::String,
                            desc: "item name".to_string(),
                            constraint: None,
                            tags: vec![],
                        },
                        Field {
                            name: "price".to_string(),
                            t: FieldType::Float32,
                            desc: "item price".to_string(),
                            constraint: Some(Constraint::from_str("@order(asc)").unwrap()),
                            tags: vec![],
                        },
                    ],
                    vec![],
                ),
                data: vec![
                    Row::from_vec(vec![
                        ("id".to_string(), Value::Int32(1001)),
                        ("name".to_string(), Value::String("Sword".to_string())),
                        ("price".to_string(), Value::Float32(5.0)),
                    ]),
                    Row::from_vec(vec![
                        ("id".to_string(), Value::Int32(1002)),
                        ("name".to_string(), Value::String("Shield".to_string())),
                        ("price".to_string(), Value::Float32(10.0)),
                    ]),
                    Row::from_vec(vec![
                        ("id".to_string(), Value::Int32(1003)),
                        ("name".to_string(), Value::String("Potion".to_string())),
                        ("price".to_string(), Value::Float32(25.0)),
                    ]),
                ],
            },
        ),
        (
            "levels".to_string(),
            Table {
                name: "levels".to_string(),
                schema: Schema::from_parts(
                    vec![
                        Field {
                            name: "level".to_string(),
                            t: FieldType::Int32,
                            desc: "level number".to_string(),
                            constraint: Some(Constraint::from_str("@seq").unwrap()),
                            tags: vec![],
                        },
                        Field {
                            name: "exp".to_string(),
                            t: FieldType::Int32,
                            desc: "experience required".to_string(),
                            constraint: None,
                            tags: vec![],
                        },
                    ],
                    vec![],
                ),
                data: vec![
                    Row::from_vec(vec![
                        ("level".to_string(), Value::Int32(1)),
                        ("exp".to_string(), Value::Int32(100)),
                    ]),
                    Row::from_vec(vec![
                        ("level".to_string(), Value::Int32(2)),
                        ("exp".to_string(), Value::Int32(250)),
                    ]),
                    Row::from_vec(vec![
                        ("level".to_string(), Value::Int32(3)),
                        ("exp".to_string(), Value::Int32(500)),
                    ]),
                ],
            },
        ),
    ]);

    Project {
        name: "game_data".to_string(),
        meta: Meta {
            version: "1.0.0".to_string(),
            hash: [0xfe; 32],
            build_at: 1700000000,
            source: vec![],
            tool: ToolVersion::default(),
        },
        tables,
    }
}

#[test]
fn test_full_pipeline_json() {
    let project = build_multi_table_project();

    // Step 1: Validate constraints
    for table in project.tables.values() {
        assert!(
            table.validate_constraints().is_ok(),
            "Constraint validation failed for table '{}'",
            table.name
        );
    }

    // Step 2: Export to JSON
    let json = Json {
        pretty: true,
        include_fields: true,
    };
    let bytes = json.to_vec(&project).unwrap();
    let s = String::from_utf8(bytes).unwrap();

    // Step 3: Verify JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed["name"], "game_data");
    assert_eq!(parsed["tables"].as_array().unwrap().len(), 2);

    // Step 4: Verify data is correct
    let items_table = &parsed["tables"][0];
    assert_eq!(items_table["name"], "items");
    assert_eq!(items_table["data"].as_array().unwrap().len(), 3);
    assert_eq!(items_table["data"][0]["name"], "Sword");

    // Step 5: Verify fields metadata
    assert!(items_table["fields"].as_array().unwrap().len() > 0);
    assert!(
        items_table["fields"][0]["tags"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("pk".to_string()))
    );
}

#[test]
fn test_full_pipeline_msgpack() {
    let project = build_multi_table_project();

    // Validate
    for table in project.tables.values() {
        assert!(table.validate_constraints().is_ok());
    }

    // Export to Msgpack and roundtrip
    let msgpack = Msgpack;
    let bytes = msgpack.to_vec(&project).unwrap();
    let decoded: Project = rmp_serde::from_slice(&bytes).unwrap();

    assert_eq!(decoded.name, "game_data");
    assert_eq!(decoded.tables.len(), 2);

    // Verify items table data survived roundtrip
    let items = &decoded.tables["items"];
    assert_eq!(items.data.len(), 3);
    assert_eq!(
        items.data[0].get_field("name").unwrap(),
        &Value::String("Sword".to_string())
    );

    let levels = &decoded.tables["levels"];
    assert_eq!(levels.data.len(), 3);
}

#[test]
fn test_full_pipeline_constraint_violation_caught() {
    // Build a project with intentional constraint violations
    let tables = IndexMap::from([(
        "bad_items".to_string(),
        Table {
            name: "bad_items".to_string(),
            schema: Schema::from_parts(
                vec![Field {
                    name: "id".to_string(),
                    t: FieldType::Int32,
                    desc: "".to_string(),
                    constraint: Some(Constraint::from_str("@unique").unwrap()),
                    tags: vec![],
                }],
                vec![],
            ),
            data: vec![
                Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
                Row::from_vec(vec![("id".to_string(), Value::Int32(1))]), // duplicate!
            ],
        },
    )]);

    let project = Project {
        name: "bad_project".to_string(),
        meta: Meta {
            version: "1.0.0".to_string(),
            hash: [0u8; 32],
            build_at: 0,
            source: vec![],
            tool: ToolVersion::default(),
        },
        tables,
    };

    // Validate should fail
    let bad_table = &project.tables["bad_items"];
    assert!(bad_table.validate_constraints().is_err());
}

#[test]
fn test_project_from_tables() {
    let tables = vec![Table {
        name: "heroes".to_string(),
        schema: Schema::from_parts(
            vec![Field {
                name: "id".to_string(),
                t: FieldType::Int32,
                desc: "".to_string(),
                constraint: None,
                tags: vec![],
            }],
            vec![],
        ),
        data: vec![Row::from_vec(vec![("id".to_string(), Value::Int32(1))])],
    }];

    let project = Project::from_tables("my_project".to_string(), tables);
    assert_eq!(project.name, "my_project");
    assert_eq!(project.tables.len(), 1);
    assert!(project.tables.contains_key("heroes"));
}

#[test]
fn test_project_multiple_tables_from_tables() {
    let tables = vec![
        Table {
            name: "t1".to_string(),
            schema: Schema::from_parts(vec![], vec![]),
            data: vec![],
        },
        Table {
            name: "t2".to_string(),
            schema: Schema::from_parts(vec![], vec![]),
            data: vec![],
        },
    ];

    let project = Project::from_tables("multi".to_string(), tables);
    assert_eq!(project.tables.len(), 2);
}

#[test]
fn test_table_with_tags() {
    let table = Table {
        name: "heroes".to_string(),
        schema: Schema::from_parts(
            vec![
                Field {
                    name: "id".to_string(),
                    t: FieldType::Int32,
                    desc: "".to_string(),
                    constraint: None,
                    tags: vec!["pk".to_string(), "auto".to_string()],
                },
                Field {
                    name: "name".to_string(),
                    t: FieldType::String,
                    desc: "".to_string(),
                    constraint: None,
                    tags: vec![],
                },
            ],
            vec![],
        ),
        data: vec![],
    };

    let json = Json {
        pretty: false,
        include_fields: true,
    };
    let project = Project::from_tables("test".to_string(), vec![table]);
    let bytes = json.to_vec(&project).unwrap();
    let s = String::from_utf8(bytes).unwrap();
    assert!(s.contains("pk"));
    assert!(s.contains("auto"));
}

#[test]
fn test_excel_read_nonexistent_file() {
    let result = read_excel("/nonexistent/path/file.xlsx");
    assert!(result.is_err());
}

// === FieldType to_type conversion (c3: width-preserving) ===

#[test]
fn test_fieldtype_to_type_basics() {
    use tablec_core::core::table::types::Type;

    assert_eq!(FieldType::Int8.to_type(), Type::Int8);
    assert_eq!(FieldType::Int16.to_type(), Type::Int16);
    assert_eq!(FieldType::Int32.to_type(), Type::Int32);
    assert_eq!(FieldType::Int64.to_type(), Type::Int64);
    assert_eq!(FieldType::Uint8.to_type(), Type::Uint8);
    assert_eq!(FieldType::Uint32.to_type(), Type::Uint32);
    assert_eq!(FieldType::Float32.to_type(), Type::Float32);
    assert_eq!(FieldType::Float64.to_type(), Type::Float64);
    assert_eq!(FieldType::String.to_type(), Type::String);
    assert_eq!(FieldType::Bool.to_type(), Type::Bool);
}

#[test]
fn test_fieldtype_to_type_array() {
    use tablec_core::core::table::types::Type;

    let ft = FieldType::Array {
        r#type: Box::new(FieldType::Int32),
    };
    assert_eq!(ft.to_type(), Type::Array(Box::new(Type::Int32)));

    let ft = FieldType::Array {
        r#type: Box::new(FieldType::Array {
            r#type: Box::new(FieldType::String),
        }),
    };
    assert_eq!(
        ft.to_type(),
        Type::Array(Box::new(Type::Array(Box::new(Type::String))))
    );
}

#[test]
fn test_fieldtype_to_type_map() {
    use tablec_core::core::table::types::Type;

    let ft = FieldType::Map {
        key: Box::new(FieldType::String),
        value: Box::new(FieldType::Int32),
    };
    assert_eq!(
        ft.to_type(),
        Type::Map(Box::new(Type::String), Box::new(Type::Int32))
    );
}

#[test]
fn test_fieldtype_to_type_struct() {
    use tablec_core::core::table::types::Type;

    let ft = FieldType::Struct {
        fields: vec![
            Field {
                name: "a".to_string(),
                t: FieldType::Int32,
                desc: "".to_string(),
                constraint: None,
                tags: vec![],
            },
            Field {
                name: "b".to_string(),
                t: FieldType::String,
                desc: "".to_string(),
                constraint: None,
                tags: vec![],
            },
        ],
    };
    match ft.to_type() {
        Type::Struct(fields) => {
            assert_eq!(fields.get("a").unwrap(), &Type::Int32);
            assert_eq!(fields.get("b").unwrap(), &Type::String);
        }
        _ => panic!("Expected Struct type"),
    }
}

// === Row operations ===

#[test]
fn test_row_get_field_exists() {
    let row = Row::from_vec(vec![
        ("id".to_string(), Value::Int32(1)),
        ("name".to_string(), Value::String("test".to_string())),
    ]);
    assert_eq!(row.get_field("id").unwrap(), &Value::Int32(1));
    assert_eq!(
        row.get_field("name").unwrap(),
        &Value::String("test".to_string())
    );
}

#[test]
fn test_row_get_field_missing() {
    let row = Row::from_vec(vec![("id".to_string(), Value::Int32(1))]);
    assert!(row.get_field("nonexistent").is_none());
}

#[test]
fn test_row_empty() {
    let row = Row::new();
    assert!(row.get_field("any").is_none());
}

#[test]
fn test_row_from_vec_empty() {
    let row = Row::from_vec(vec![]);
    assert!(row.get_field("any").is_none());
}

// === Table with complex types ===

#[test]
fn test_table_with_nested_types_json_roundtrip() {
    let mut struct_fields = HashMap::new();
    struct_fields.insert(
        "x".to_string(),
        tablec_core::core::table::types::Type::Int32,
    );
    struct_fields.insert(
        "y".to_string(),
        tablec_core::core::table::types::Type::Int32,
    );

    let mut s = IndexMap::new();
    s.insert("x".to_string(), Value::Int32(10));
    s.insert("y".to_string(), Value::Int32(20));

    let table = Table {
        name: "test".to_string(),
        schema: Schema::from_parts(
            vec![Field {
                name: "pos".to_string(),
                t: FieldType::Struct {
                    fields: vec![
                        Field {
                            name: "x".to_string(),
                            t: FieldType::Int32,
                            desc: "".to_string(),
                            constraint: None,
                            tags: vec![],
                        },
                        Field {
                            name: "y".to_string(),
                            t: FieldType::Int32,
                            desc: "".to_string(),
                            constraint: None,
                            tags: vec![],
                        },
                    ],
                },
                desc: "".to_string(),
                constraint: None,
                tags: vec![],
            }],
            vec![],
        ),
        data: vec![Row::from_vec(vec![("pos".to_string(), Value::Struct(s))])],
    };

    let json = Json {
        pretty: true,
        include_fields: false,
    };
    let project = Project::from_tables("test".to_string(), vec![table]);
    let bytes = json.to_vec(&project).unwrap();
    let s = String::from_utf8(bytes).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();

    let pos = &parsed["tables"][0]["data"][0]["pos"];
    assert_eq!(pos["x"], 10);
    assert_eq!(pos["y"], 20);
}
