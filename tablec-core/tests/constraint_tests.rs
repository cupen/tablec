use tablec_core::core::table::constraint::Constraint;
use tablec_core::core::table::field::{Field, FieldType};
use tablec_core::core::table::row::Row;
use tablec_core::core::table::value::Value;
use tablec_core::core::table::table::Table;
use std::str::FromStr;

// === Constraint Parsing ===

#[test]
fn test_parse_constraint_no_args() {
    let c = Constraint::from_str("@unique").unwrap();
    assert_eq!(c.func, "unique");
    assert!(c.args.is_empty());
}

#[test]
fn test_parse_constraint_with_args() {
    let c = Constraint::from_str("@unique(name1, name2)").unwrap();
    assert_eq!(c.func, "unique");
    assert_eq!(c.args, vec!["name1", "name2"]);
}

#[test]
fn test_parse_constraint_single_arg() {
    let c = Constraint::from_str("@seq(2)").unwrap();
    assert_eq!(c.func, "seq");
    assert_eq!(c.args, vec!["2"]);
}

#[test]
fn test_parse_constraint_order_desc() {
    let c = Constraint::from_str("@order(desc)").unwrap();
    assert_eq!(c.func, "order");
    assert_eq!(c.args, vec!["desc"]);
}

#[test]
fn test_parse_constraint_order_asc() {
    let c = Constraint::from_str("@order(asc)").unwrap();
    assert_eq!(c.func, "order");
    assert_eq!(c.args, vec!["asc"]);
}

#[test]
fn test_parse_constraint_no_at() {
    assert!(Constraint::from_str("unique").is_err());
    assert!(Constraint::from_str("func(arg)").is_err());
}

#[test]
fn test_parse_constraint_empty() {
    assert!(Constraint::from_str("").is_err());
}

#[test]
fn test_parse_constraint_only_at() {
    assert!(Constraint::from_str("@").is_err());
}

#[test]
fn test_parse_constraint_unclosed_paren() {
    assert!(Constraint::from_str("@func(arg1, arg2").is_err());
}

#[test]
fn test_parse_constraint_space_no_paren() {
    assert!(Constraint::from_str("@func arg").is_err());
}


// === @unique validation ===

fn make_field(name: &str, t: FieldType) -> Field {
    Field {
        name: name.to_string(),
        t,
        desc: "".to_string(),
        constraint: None,
        tags: vec![],
    }
}

#[test]
fn test_unique_single_field_pass() {
    let c = Constraint::from_str("@unique").unwrap();
    let fields = vec![make_field("id", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("id".to_string(), Value::Int32(2))]),
        Row::from_vec(vec![("id".to_string(), Value::Int32(3))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_unique_single_field_fail() {
    let c = Constraint::from_str("@unique").unwrap();
    let fields = vec![make_field("id", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("id".to_string(), Value::Int32(2))]),
        Row::from_vec(vec![("id".to_string(), Value::Int32(1))]), // duplicate
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

#[test]
fn test_unique_single_field_all_duplicate() {
    let c = Constraint::from_str("@unique").unwrap();
    let fields = vec![make_field("id", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

#[test]
fn test_unique_empty_rows() {
    let c = Constraint::from_str("@unique").unwrap();
    let fields = vec![make_field("id", FieldType::Int32)];
    let rows: Vec<Row> = vec![];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_unique_single_row() {
    let c = Constraint::from_str("@unique").unwrap();
    let fields = vec![make_field("id", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_unique_string_field() {
    let c = Constraint::from_str("@unique").unwrap();
    let fields = vec![make_field("name", FieldType::String)];
    let rows = vec![
        Row::from_vec(vec![("name".to_string(), Value::String("Alice".to_string()))]),
        Row::from_vec(vec![("name".to_string(), Value::String("Bob".to_string()))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_unique_string_field_duplicate() {
    let c = Constraint::from_str("@unique").unwrap();
    let fields = vec![make_field("name", FieldType::String)];
    let rows = vec![
        Row::from_vec(vec![("name".to_string(), Value::String("Alice".to_string()))]),
        Row::from_vec(vec![("name".to_string(), Value::String("Alice".to_string()))]),
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

#[test]
fn test_unique_composite_pass() {
    let c = Constraint::from_str("@unique(a, b)").unwrap();
    let fields = vec![
        make_field("a", FieldType::Int32),
        make_field("b", FieldType::Int32),
    ];
    let rows = vec![
        Row::from_vec(vec![
            ("a".to_string(), Value::Int32(1)),
            ("b".to_string(), Value::Int32(1)),
        ]),
        Row::from_vec(vec![
            ("a".to_string(), Value::Int32(1)),
            ("b".to_string(), Value::Int32(2)),
        ]),
        Row::from_vec(vec![
            ("a".to_string(), Value::Int32(2)),
            ("b".to_string(), Value::Int32(1)),
        ]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_unique_composite_fail() {
    let c = Constraint::from_str("@unique(a, b)").unwrap();
    let fields = vec![
        make_field("a", FieldType::Int32),
        make_field("b", FieldType::Int32),
    ];
    let rows = vec![
        Row::from_vec(vec![
            ("a".to_string(), Value::Int32(1)),
            ("b".to_string(), Value::Int32(1)),
        ]),
        Row::from_vec(vec![
            ("a".to_string(), Value::Int32(1)),
            ("b".to_string(), Value::Int32(1)),
        ]),
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

// === @seq validation ===

#[test]
fn test_seq_default_step_pass() {
    let c = Constraint::from_str("@seq").unwrap();
    let fields = vec![make_field("seq", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(2))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(3))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(4))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_seq_default_step_fail() {
    let c = Constraint::from_str("@seq").unwrap();
    let fields = vec![make_field("seq", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(3))]), // skip 2
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

#[test]
fn test_seq_custom_step() {
    let c = Constraint::from_str("@seq(2)").unwrap();
    let fields = vec![make_field("seq", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(3))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(5))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_seq_custom_step_fail() {
    let c = Constraint::from_str("@seq(2)").unwrap();
    let fields = vec![make_field("seq", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(2))]), // should be 3
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

#[test]
fn test_seq_negative_step() {
    let c = Constraint::from_str("@seq(-1)").unwrap();
    let fields = vec![make_field("seq", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(0))]),
        Row::from_vec(vec![("seq".to_string(), Value::Int32(-1))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_seq_uint_field() {
    let c = Constraint::from_str("@seq").unwrap();
    let fields = vec![make_field("seq", FieldType::Uint32)];
    let rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::Uint32(1))]),
        Row::from_vec(vec![("seq".to_string(), Value::Uint32(2))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_seq_empty_rows() {
    let c = Constraint::from_str("@seq").unwrap();
    let fields = vec![make_field("seq", FieldType::Int32)];
    let rows: Vec<Row> = vec![];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_seq_non_numeric_field() {
    let c = Constraint::from_str("@seq").unwrap();
    let fields = vec![make_field("seq", FieldType::String)];
    let rows = vec![
        Row::from_vec(vec![("seq".to_string(), Value::String("a".to_string()))]),
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

// === @order validation ===

#[test]
fn test_order_asc_pass() {
    let c = Constraint::from_str("@order(asc)").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(2))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(5))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(10))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_order_asc_fail() {
    let c = Constraint::from_str("@order(asc)").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(5))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(3))]), // violates asc
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

#[test]
fn test_order_desc_pass() {
    let c = Constraint::from_str("@order(desc)").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Int32(10))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(5))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(1))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_order_desc_fail() {
    let c = Constraint::from_str("@order(desc)").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Int32(10))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(5))]), // violates desc
    ];
    assert!(c.validate(&fields, &rows).is_err());
}

#[test]
fn test_order_default_asc() {
    let c = Constraint::from_str("@order").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(2))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_order_float_values() {
    let c = Constraint::from_str("@order(asc)").unwrap();
    let fields = vec![make_field("val", FieldType::Float32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Float32(1.0))]),
        Row::from_vec(vec![("val".to_string(), Value::Float32(2.5))]),
        Row::from_vec(vec![("val".to_string(), Value::Float32(3.0))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_order_equal_values_pass() {
    let c = Constraint::from_str("@order(asc)").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(1))]),
        Row::from_vec(vec![("val".to_string(), Value::Int32(2))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_order_empty_rows() {
    let c = Constraint::from_str("@order(asc)").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows: Vec<Row> = vec![];
    assert!(c.validate(&fields, &rows).is_ok());
}

#[test]
fn test_order_single_row() {
    let c = Constraint::from_str("@order(asc)").unwrap();
    let fields = vec![make_field("val", FieldType::Int32)];
    let rows = vec![
        Row::from_vec(vec![("val".to_string(), Value::Int32(42))]),
    ];
    assert!(c.validate(&fields, &rows).is_ok());
}

// === ConstraintValidator (table-level) ===

#[test]
fn test_validator_all_pass() {
    let table = Table {
        name: "test".to_string(),
        fields: vec![
            Field {
                name: "id".to_string(),
                t: FieldType::Int32,
                desc: "".to_string(),
                constraint: Some(Constraint::from_str("@unique").unwrap()),
                tags: vec![],
            },
            Field {
                name: "seq".to_string(),
                t: FieldType::Int32,
                desc: "".to_string(),
                constraint: Some(Constraint::from_str("@seq").unwrap()),
                tags: vec![],
            },
        ],
        data: vec![
            Row::from_vec(vec![
                ("id".to_string(), Value::Int32(1)),
                ("seq".to_string(), Value::Int32(1)),
            ]),
            Row::from_vec(vec![
                ("id".to_string(), Value::Int32(2)),
                ("seq".to_string(), Value::Int32(2)),
            ]),
        ],
        constraints: vec![],
    };
    assert!(table.validate_constraints().is_ok());
}

#[test]
fn test_validator_one_fails() {
    let table = Table {
        name: "test".to_string(),
        fields: vec![
            Field {
                name: "id".to_string(),
                t: FieldType::Int32,
                desc: "".to_string(),
                constraint: Some(Constraint::from_str("@unique").unwrap()),
                tags: vec![],
            },
        ],
        data: vec![
            Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
        ],
        constraints: vec![],
    };
    let result = table.validate_constraints();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_validator_no_constraints() {
    let table = Table {
        name: "test".to_string(),
        fields: vec![
            make_field("name", FieldType::String),
        ],
        data: vec![
            Row::from_vec(vec![("name".to_string(), Value::String("Alice".to_string()))]),
        ],
        constraints: vec![],
    };
    assert!(table.validate_constraints().is_ok());
}

#[test]
fn test_validator_unknown_constraint_function() {
    let table = Table {
        name: "test".to_string(),
        fields: vec![
            Field {
                name: "id".to_string(),
                t: FieldType::Int32,
                desc: "".to_string(),
                constraint: Some(Constraint { func: "nonexistent".to_string(), args: vec![], location: Default::default() }),
                tags: vec![],
            },
        ],
        data: vec![
            Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
        ],
        constraints: vec![],
    };
    assert!(table.validate_constraints().is_err());
}
