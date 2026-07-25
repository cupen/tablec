use tablec_core::core::project::project::Project;

fn build_project(rows: Vec<Vec<(&str, tablec_core::core::table::value::Value)>>) -> Project {
    use tablec_core::core::table::field::{Field, FieldType};
    use tablec_core::core::table::row::Row;
    use tablec_core::core::table::table::Table;
    let field_a = Field {
        name: "a".into(),
        t: FieldType::Int32,
        desc: "".into(),
        constraint: None,
        tags: vec![],
    };
    let field_b = Field {
        name: "b".into(),
        t: FieldType::String,
        desc: "".into(),
        constraint: None,
        tags: vec![],
    };
    let data = rows
        .into_iter()
        .map(|r| {
            Row::from_vec(
                r.into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    Project::from_tables(
        "p".into(),
        vec![Table {
            name: "S".into(),
            fields: vec![field_a, field_b],
            data,
            constraints: vec![],
        }],
    )
}

#[test]
fn hash_is_stable_across_two_runs() {
    let mut p1 = build_project(vec![vec![
        ("a", tablec_core::core::table::value::Value::Int32(1)),
        (
            "b",
            tablec_core::core::table::value::Value::String("x".into()),
        ),
    ]]);
    let mut p2 = build_project(vec![vec![
        ("a", tablec_core::core::table::value::Value::Int32(1)),
        (
            "b",
            tablec_core::core::table::value::Value::String("x".into()),
        ),
    ]]);
    p1.calculate_hash();
    p2.calculate_hash();
    assert_eq!(p1.meta.hash, p2.meta.hash);
}

#[test]
fn hash_changes_when_rows_reordered() {
    let mut p1 = build_project(vec![
        vec![
            ("a", tablec_core::core::table::value::Value::Int32(1)),
            (
                "b",
                tablec_core::core::table::value::Value::String("a".into()),
            ),
        ],
        vec![
            ("a", tablec_core::core::table::value::Value::Int32(2)),
            (
                "b",
                tablec_core::core::table::value::Value::String("b".into()),
            ),
        ],
    ]);
    let mut p2 = build_project(vec![
        vec![
            ("a", tablec_core::core::table::value::Value::Int32(2)),
            (
                "b",
                tablec_core::core::table::value::Value::String("b".into()),
            ),
        ],
        vec![
            ("a", tablec_core::core::table::value::Value::Int32(1)),
            (
                "b",
                tablec_core::core::table::value::Value::String("a".into()),
            ),
        ],
    ]);
    p1.calculate_hash();
    p2.calculate_hash();
    assert_ne!(p1.meta.hash, p2.meta.hash);
}

#[test]
fn hash_changes_when_row_deleted() {
    let mut p1 = build_project(vec![vec![
        ("a", tablec_core::core::table::value::Value::Int32(1)),
        (
            "b",
            tablec_core::core::table::value::Value::String("x".into()),
        ),
    ]]);
    let mut p2 = build_project(vec![
        vec![
            ("a", tablec_core::core::table::value::Value::Int32(1)),
            (
                "b",
                tablec_core::core::table::value::Value::String("x".into()),
            ),
        ],
        vec![
            ("a", tablec_core::core::table::value::Value::Int32(2)),
            (
                "b",
                tablec_core::core::table::value::Value::String("y".into()),
            ),
        ],
    ]);
    p1.calculate_hash();
    p2.calculate_hash();
    assert_ne!(p1.meta.hash, p2.meta.hash);
}
