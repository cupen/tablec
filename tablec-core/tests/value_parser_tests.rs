use tablec_core::core::diagnostic::SourceLocation;
use tablec_core::core::parser::value_parser::parse_value;
use tablec_core::core::table::field::{Field, FieldType};
use tablec_core::core::table::value::Value;

fn loc() -> SourceLocation { SourceLocation::default() }

// === Basic types: happy path ===

#[test]
fn test_parse_int_positive() {
    assert_eq!(parse_value("42", &FieldType::Int32, loc()).unwrap(), Value::Int32(42));
}

#[test]
fn test_parse_int_negative() {
    assert_eq!(parse_value("-7", &FieldType::Int32, loc()).unwrap(), Value::Int32(-7));
}

#[test]
fn test_parse_int_zero() {
    assert_eq!(parse_value("0", &FieldType::Int32, loc()).unwrap(), Value::Int32(0));
}

#[test]
fn test_parse_int_large() {
    assert_eq!(parse_value("9223372036854775807", &FieldType::Int64, loc()).unwrap(),
        Value::Int64(9223372036854775807));
}

#[test]
fn test_parse_uint() {
    assert_eq!(parse_value("42", &FieldType::Uint32, loc()).unwrap(), Value::Uint32(42));
}

#[test]
fn test_parse_uint_zero() {
    assert_eq!(parse_value("0", &FieldType::Uint32, loc()).unwrap(), Value::Uint32(0));
}

#[test]
fn test_parse_float_positive() {
    let v = parse_value("3.14", &FieldType::Float64, loc()).unwrap();
    assert!(matches!(v, Value::Float64(f) if (f - 3.14).abs() < 1e-10));
}

#[test]
fn test_parse_float_negative() {
    let v = parse_value("-0.5", &FieldType::Float64, loc()).unwrap();
    assert!(matches!(v, Value::Float64(f) if (f + 0.5).abs() < 1e-10));
}

#[test]
fn test_parse_float_integer_form() {
    let v = parse_value("10", &FieldType::Float32, loc()).unwrap();
    assert!(matches!(v, Value::Float32(f) if (f - 10.0).abs() < 1e-10));
}

#[test]
fn test_parse_string_plain() {
    assert_eq!(parse_value("hello", &FieldType::String, loc()).unwrap(),
        Value::String("hello".to_string()));
}

#[test]
fn test_parse_string_quoted_single() {
    assert_eq!(parse_value("'hello'", &FieldType::String, loc()).unwrap(),
        Value::String("hello".to_string()));
}

#[test]
fn test_parse_string_quoted_double() {
    assert_eq!(parse_value("\"world\"", &FieldType::String, loc()).unwrap(),
        Value::String("world".to_string()));
}

#[test]
fn test_parse_string_empty() {
    assert_eq!(parse_value("", &FieldType::String, loc()).unwrap(),
        Value::String("".to_string()));
}

#[test]
fn test_parse_bool_true() {
    assert_eq!(parse_value("true", &FieldType::Bool, loc()).unwrap(), Value::Bool(true));
}

#[test]
fn test_parse_bool_false() {
    assert_eq!(parse_value("false", &FieldType::Bool, loc()).unwrap(), Value::Bool(false));
}

// === Basic types: error cases ===

#[test]
fn test_parse_int_invalid() {
    assert!(parse_value("abc", &FieldType::Int32, loc()).is_err());
    assert!(parse_value("1.5", &FieldType::Int32, loc()).is_err());
    assert!(parse_value("", &FieldType::Int32, loc()).is_err());
}

#[test]
fn test_parse_int_overflow() {
    // Value larger than i32 max
    assert!(parse_value("99999999999999999999", &FieldType::Int32, loc()).is_err());
}

#[test]
fn test_parse_uint_negative() {
    assert!(parse_value("-1", &FieldType::Uint32, loc()).is_err());
}

#[test]
fn test_parse_uint_invalid() {
    assert!(parse_value("abc", &FieldType::Uint32, loc()).is_err());
}

#[test]
fn test_parse_float_invalid() {
    assert!(parse_value("abc", &FieldType::Float64, loc()).is_err());
    assert!(parse_value("", &FieldType::Float64, loc()).is_err());
}

#[test]
fn test_parse_bool_invalid() {
    // post-c3: "1"/"0" accepted as bool too (brief value_parser step 4)
    assert!(parse_value("yes", &FieldType::Bool, loc()).is_err());
    assert!(parse_value("", &FieldType::Bool, loc()).is_err());
}

// === Array parsing ===

#[test]
fn test_parse_array_int() {
    let v = parse_value("[1, 2, 3]", &FieldType::Array { r#type: Box::new(FieldType::Int32) }, loc()).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::Int32(1), Value::Int32(2), Value::Int32(3),
    ]));
}

#[test]
fn test_parse_array_string() {
    let v = parse_value("[hello, world]", &FieldType::Array { r#type: Box::new(FieldType::String) }, loc()).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::String("hello".to_string()),
        Value::String("world".to_string()),
    ]));
}

#[test]
fn test_parse_array_empty() {
    let v = parse_value("[]", &FieldType::Array { r#type: Box::new(FieldType::Int32) }, loc()).unwrap();
    assert_eq!(v, Value::Array(vec![]));
}

#[test]
fn test_parse_array_single() {
    let v = parse_value("[42]", &FieldType::Array { r#type: Box::new(FieldType::Int32) }, loc()).unwrap();
    assert_eq!(v, Value::Array(vec![Value::Int32(42)]));
}

#[test]
fn test_parse_array_nested() {
    let inner = FieldType::Array { r#type: Box::new(FieldType::Int32) };
    let v = parse_value("[[1,2],[3,4]]", &FieldType::Array { r#type: Box::new(inner) }, loc()).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::Array(vec![Value::Int32(1), Value::Int32(2)]),
        Value::Array(vec![Value::Int32(3), Value::Int32(4)]),
    ]));
}

#[test]
fn test_parse_array_with_spaces() {
    let v = parse_value("[ 1 , 2 , 3 ]", &FieldType::Array { r#type: Box::new(FieldType::Int32) }, loc()).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::Int32(1), Value::Int32(2), Value::Int32(3),
    ]));
}

#[test]
fn test_parse_array_invalid_format() {
    let ty = &FieldType::Array { r#type: Box::new(FieldType::Int32) };
    assert!(parse_value("1,2,3", ty, loc()).is_err());
    assert!(parse_value("[1,2,3", ty, loc()).is_err());
    assert!(parse_value("1,2,3]", ty, loc()).is_err());
}

#[test]
fn test_parse_array_wrong_inner_type() {
    // Elements must be parseable as the declared inner type
    let v = parse_value("[abc, def]", &FieldType::Array { r#type: Box::new(FieldType::Int32) }, loc());
    assert!(v.is_err());
}

// === Map parsing ===

#[test]
fn test_parse_map_int_to_string() {
    let ty = FieldType::Map { key: Box::new(FieldType::Int32), value: Box::new(FieldType::String) };
    let v = parse_value("1:one, 2:two", &ty, loc()).unwrap();
    match v {
        Value::Map(m) => {
            assert_eq!(m.get(&Value::Int32(1)).unwrap(), &Value::String("one".to_string()));
            assert_eq!(m.get(&Value::Int32(2)).unwrap(), &Value::String("two".to_string()));
        }
        _ => panic!("Expected Map"),
    }
}

#[test]
fn test_parse_map_string_to_int() {
    let ty = FieldType::Map { key: Box::new(FieldType::String), value: Box::new(FieldType::Int32) };
    let v = parse_value("k1:1, k2:2", &ty, loc()).unwrap();
    match v {
        Value::Map(m) => {
            assert_eq!(m.get(&Value::String("k1".to_string())).unwrap(), &Value::Int32(1));
            assert_eq!(m.get(&Value::String("k2".to_string())).unwrap(), &Value::Int32(2));
        }
        _ => panic!("Expected Map"),
    }
}

#[test]
fn test_parse_map_empty() {
    let ty = FieldType::Map { key: Box::new(FieldType::String), value: Box::new(FieldType::Int32) };
    let v = parse_value("", &ty, loc()).unwrap();
    assert!(matches!(v, Value::Map(m) if m.is_empty()));
}

#[test]
fn test_parse_map_single() {
    let ty = FieldType::Map { key: Box::new(FieldType::String), value: Box::new(FieldType::String) };
    let v = parse_value("key:value", &ty, loc()).unwrap();
    match v {
        Value::Map(m) => {
            assert_eq!(m.len(), 1);
        }
        _ => panic!("Expected Map"),
    }
}

#[test]
fn test_parse_map_invalid_format() {
    let ty = FieldType::Map { key: Box::new(FieldType::String), value: Box::new(FieldType::Int32) };
    let v = parse_value("key", &ty, loc());
    assert!(v.is_err());
}

// === Struct parsing (post-c3: by-name matching) ===

#[test]
fn test_parse_struct_simple() {
    let fields = vec![
        Field { name: "x".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
        Field { name: "y".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
    ];
    let ty = FieldType::Struct { fields };
    let v = parse_value("{x: 100, y: 200}", &ty, loc()).unwrap();
    match v {
        Value::Struct(s) => {
            assert_eq!(s.get("x"), Some(&Value::Int32(100)));
            assert_eq!(s.get("y"), Some(&Value::Int32(200)));
        }
        _ => panic!("Expected Struct"),
    }
}

#[test]
fn test_parse_struct_mixed_types() {
    // Order doesn't matter with by-name matching.
    let fields = vec![
        Field { name: "a".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
        Field { name: "b".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
    ];
    let ty = FieldType::Struct { fields };
    let v = parse_value("{b: 77, a: 42}", &ty, loc()).unwrap();
    match v {
        Value::Struct(s) => {
            assert_eq!(s.get("a"), Some(&Value::Int32(42)));
            assert_eq!(s.get("b"), Some(&Value::Int32(77)));
        }
        _ => panic!("Expected Struct"),
    }
}

#[test]
fn test_parse_struct_wrong_field_count() {
    let fields = vec![
        Field { name: "x".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
        Field { name: "y".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
    ];
    // Missing y - yields StructFieldCountMismatch
    let ty = FieldType::Struct { fields };
    let err = parse_value("{x: 1}", &ty, loc()).unwrap_err();
    assert_eq!(err.code, tablec_core::core::diagnostic::DiagnosticCode::StructFieldCountMismatch);
}

#[test]
fn test_parse_struct_invalid_format_no_braces() {
    let fields = vec![
        Field { name: "x".into(), t: FieldType::Int32, desc: "".into(), constraint: None, tags: vec![] },
    ];
    let ty = FieldType::Struct { fields };
    assert!(parse_value("1", &ty, loc()).is_err());
}

// === Whitespace handling ===

#[test]
fn test_parse_with_leading_trailing_spaces() {
    assert_eq!(parse_value("  42  ", &FieldType::Int32, loc()).unwrap(), Value::Int32(42));
    assert_eq!(parse_value("  hello  ", &FieldType::String, loc()).unwrap(),
        Value::String("hello".to_string()));
}
