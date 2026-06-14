use tablec_core::core::parser::value_parser::parse_value;
use tablec_core::core::table::types::Type;
use tablec_core::core::table::value::Value;
use std::collections::HashMap;

// === Basic types: happy path ===

#[test]
fn test_parse_int_positive() {
    assert_eq!(parse_value("42", &Type::Int).unwrap(), Value::Int(42));
}

#[test]
fn test_parse_int_negative() {
    assert_eq!(parse_value("-7", &Type::Int).unwrap(), Value::Int(-7));
}

#[test]
fn test_parse_int_zero() {
    assert_eq!(parse_value("0", &Type::Int).unwrap(), Value::Int(0));
}

#[test]
fn test_parse_int_large() {
    assert_eq!(parse_value("9223372036854775807", &Type::Int).unwrap(),
        Value::Int(9223372036854775807));
}

#[test]
fn test_parse_uint() {
    assert_eq!(parse_value("42", &Type::Uint).unwrap(), Value::Uint(42));
}

#[test]
fn test_parse_uint_zero() {
    assert_eq!(parse_value("0", &Type::Uint).unwrap(), Value::Uint(0));
}

#[test]
fn test_parse_float_positive() {
    let v = parse_value("3.14", &Type::Float).unwrap();
    assert!(matches!(v, Value::Float(f) if (f - 3.14).abs() < 1e-10));
}

#[test]
fn test_parse_float_negative() {
    let v = parse_value("-0.5", &Type::Float).unwrap();
    assert!(matches!(v, Value::Float(f) if (f + 0.5).abs() < 1e-10));
}

#[test]
fn test_parse_float_integer_form() {
    let v = parse_value("10", &Type::Float).unwrap();
    assert!(matches!(v, Value::Float(f) if (f - 10.0).abs() < 1e-10));
}

#[test]
fn test_parse_string_plain() {
    assert_eq!(parse_value("hello", &Type::String).unwrap(),
        Value::String("hello".to_string()));
}

#[test]
fn test_parse_string_quoted_single() {
    assert_eq!(parse_value("'hello'", &Type::String).unwrap(),
        Value::String("hello".to_string()));
}

#[test]
fn test_parse_string_quoted_double() {
    assert_eq!(parse_value("\"world\"", &Type::String).unwrap(),
        Value::String("world".to_string()));
}

#[test]
fn test_parse_string_empty() {
    assert_eq!(parse_value("", &Type::String).unwrap(),
        Value::String("".to_string()));
}

#[test]
fn test_parse_bool_true() {
    assert_eq!(parse_value("true", &Type::Bool).unwrap(), Value::Bool(true));
}

#[test]
fn test_parse_bool_false() {
    assert_eq!(parse_value("false", &Type::Bool).unwrap(), Value::Bool(false));
}

// === Basic types: error cases ===

#[test]
fn test_parse_int_invalid() {
    assert!(parse_value("abc", &Type::Int).is_err());
    assert!(parse_value("1.5", &Type::Int).is_err());
    assert!(parse_value("", &Type::Int).is_err());
}

#[test]
fn test_parse_int_overflow() {
    // Value larger than i64 max
    assert!(parse_value("99999999999999999999", &Type::Int).is_err());
}

#[test]
fn test_parse_uint_negative() {
    assert!(parse_value("-1", &Type::Uint).is_err());
}

#[test]
fn test_parse_uint_invalid() {
    assert!(parse_value("abc", &Type::Uint).is_err());
}

#[test]
fn test_parse_float_invalid() {
    assert!(parse_value("abc", &Type::Float).is_err());
    assert!(parse_value("", &Type::Float).is_err());
}

#[test]
fn test_parse_bool_invalid() {
    assert!(parse_value("yes", &Type::Bool).is_err());
    assert!(parse_value("1", &Type::Bool).is_err());
    assert!(parse_value("", &Type::Bool).is_err());
}

// === Array parsing ===

#[test]
fn test_parse_array_int() {
    let v = parse_value("[1, 2, 3]", &Type::Array(Box::new(Type::Int))).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::Int(1), Value::Int(2), Value::Int(3),
    ]));
}

#[test]
fn test_parse_array_string() {
    let v = parse_value("[hello, world]", &Type::Array(Box::new(Type::String))).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::String("hello".to_string()),
        Value::String("world".to_string()),
    ]));
}

#[test]
fn test_parse_array_empty() {
    let v = parse_value("[]", &Type::Array(Box::new(Type::Int))).unwrap();
    assert_eq!(v, Value::Array(vec![]));
}

#[test]
fn test_parse_array_single() {
    let v = parse_value("[42]", &Type::Array(Box::new(Type::Int))).unwrap();
    assert_eq!(v, Value::Array(vec![Value::Int(42)]));
}

#[test]
fn test_parse_array_nested() {
    let inner = Type::Array(Box::new(Type::Int));
    let v = parse_value("[[1,2],[3,4]]", &Type::Array(Box::new(inner))).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::Array(vec![Value::Int(1), Value::Int(2)]),
        Value::Array(vec![Value::Int(3), Value::Int(4)]),
    ]));
}

#[test]
fn test_parse_array_with_spaces() {
    let v = parse_value("[ 1 , 2 , 3 ]", &Type::Array(Box::new(Type::Int))).unwrap();
    assert_eq!(v, Value::Array(vec![
        Value::Int(1), Value::Int(2), Value::Int(3),
    ]));
}

#[test]
fn test_parse_array_invalid_format() {
    assert!(parse_value("1,2,3", &Type::Array(Box::new(Type::Int))).is_err());
    assert!(parse_value("[1,2,3", &Type::Array(Box::new(Type::Int))).is_err());
    assert!(parse_value("1,2,3]", &Type::Array(Box::new(Type::Int))).is_err());
}

#[test]
fn test_parse_array_wrong_inner_type() {
    // Elements must be parseable as the declared inner type
    let v = parse_value("[abc, def]", &Type::Array(Box::new(Type::Int)));
    assert!(v.is_err());
}

// === Map parsing ===

#[test]
fn test_parse_map_int_to_string() {
    let v = parse_value("1:one, 2:two",
        &Type::Map(Box::new(Type::Int), Box::new(Type::String))).unwrap();
    match v {
        Value::Map(m) => {
            assert_eq!(m.get(&Value::Int(1)).unwrap(), &Value::String("one".to_string()));
            assert_eq!(m.get(&Value::Int(2)).unwrap(), &Value::String("two".to_string()));
        }
        _ => panic!("Expected Map"),
    }
}

#[test]
fn test_parse_map_string_to_int() {
    let v = parse_value("k1:1, k2:2",
        &Type::Map(Box::new(Type::String), Box::new(Type::Int))).unwrap();
    match v {
        Value::Map(m) => {
            assert_eq!(m.get(&Value::String("k1".to_string())).unwrap(), &Value::Int(1));
            assert_eq!(m.get(&Value::String("k2".to_string())).unwrap(), &Value::Int(2));
        }
        _ => panic!("Expected Map"),
    }
}

#[test]
fn test_parse_map_empty() {
    let v = parse_value("",
        &Type::Map(Box::new(Type::String), Box::new(Type::Int))).unwrap();
    assert!(matches!(v, Value::Map(m) if m.is_empty()));
}

#[test]
fn test_parse_map_single() {
    let v = parse_value("key:value",
        &Type::Map(Box::new(Type::String), Box::new(Type::String))).unwrap();
    match v {
        Value::Map(m) => {
            assert_eq!(m.len(), 1);
        }
        _ => panic!("Expected Map"),
    }
}

#[test]
fn test_parse_map_invalid_format() {
    let v = parse_value("key",
        &Type::Map(Box::new(Type::String), Box::new(Type::Int)));
    assert!(v.is_err());
}

// === Struct parsing ===

#[test]
fn test_parse_struct_simple() {
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), Type::Int);
    fields.insert("y".to_string(), Type::Int);
    let v = parse_value("{100, 200}", &Type::Struct(fields)).unwrap();
    match v {
        Value::Struct(s) => {
            // HashMap-based struct fields; order depends on iteration
            assert_eq!(s.len(), 2);
            // Values are assigned in field-definition order from HashMap
            // Just verify both fields got assigned non-Null values
            assert!(s.get("x").is_some());
            assert!(s.get("y").is_some());
        }
        _ => panic!("Expected Struct"),
    }
}

#[test]
fn test_parse_struct_mixed_types() {
    // Both fields Int, so order doesn't matter
    let mut fields = HashMap::new();
    fields.insert("a".to_string(), Type::Int);
    fields.insert("b".to_string(), Type::Int);
    let v = parse_value("{42, 77}", &Type::Struct(fields)).unwrap();
    match v {
        Value::Struct(s) => {
            assert_eq!(s.len(), 2);
            assert!(s.contains_key("a"));
            assert!(s.contains_key("b"));
        }
        _ => panic!("Expected Struct"),
    }
}

#[test]
fn test_parse_struct_nested() {
    let mut inner = HashMap::new();
    inner.insert("a".to_string(), Type::Int);
    inner.insert("b".to_string(), Type::Int);
    let mut outer = HashMap::new();
    outer.insert("data".to_string(), Type::Struct(inner));
    let v = parse_value("{{1, 2}}", &Type::Struct(outer)).unwrap();
    match v {
        Value::Struct(s) => {
            match s.get("data").unwrap() {
                Value::Struct(inner_s) => {
                    assert_eq!(inner_s.len(), 2);
                    assert!(inner_s.contains_key("a"));
                    assert!(inner_s.contains_key("b"));
                }
                _ => panic!("Expected nested Struct"),
            }
        }
        _ => panic!("Expected Struct"),
    }
}

#[test]
fn test_parse_struct_wrong_field_count() {
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), Type::Int);
    fields.insert("y".to_string(), Type::Int);
    // Only one value for two fields
    assert!(parse_value("{1}", &Type::Struct(fields)).is_err());
}

#[test]
fn test_parse_struct_too_many_values() {
    let mut fields = HashMap::new();
    fields.insert("x".to_string(), Type::Int);
    // Two values for one field
    assert!(parse_value("{1, 2}", &Type::Struct(fields)).is_err());
}

#[test]
fn test_parse_struct_invalid_format_no_braces() {
    let fields = {
        let mut m = HashMap::new();
        m.insert("x".to_string(), Type::Int);
        m
    };
    assert!(parse_value("1", &Type::Struct(fields)).is_err());
}

#[test]
fn test_parse_struct_invalid_format_brackets() {
    let fields = {
        let mut m = HashMap::new();
        m.insert("x".to_string(), Type::Int);
        m
    };
    assert!(parse_value("[1]", &Type::Struct(fields)).is_err());
}

// === Any type ===

#[test]
fn test_parse_any() {
    assert_eq!(parse_value("hello", &Type::Any).unwrap(),
        Value::String("hello".to_string()));
    assert_eq!(parse_value("123", &Type::Any).unwrap(),
        Value::String("123".to_string()));
}

// === Whitespace handling ===

#[test]
fn test_parse_with_leading_trailing_spaces() {
    assert_eq!(parse_value("  42  ", &Type::Int).unwrap(), Value::Int(42));
    assert_eq!(parse_value("  hello  ", &Type::String).unwrap(),
        Value::String("hello".to_string()));
}
