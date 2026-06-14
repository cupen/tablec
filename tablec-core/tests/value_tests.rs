use tablec_core::core::table::value::Value;
use indexmap::IndexMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

// === Display ===

#[test]
fn test_display_int() {
    assert_eq!(Value::Int(42).to_string(), "42");
    assert_eq!(Value::Int(-7).to_string(), "-7");
    assert_eq!(Value::Int(0).to_string(), "0");
}

#[test]
fn test_display_uint() {
    assert_eq!(Value::Uint(0).to_string(), "0");
    assert_eq!(Value::Uint(999).to_string(), "999");
}

#[test]
fn test_display_float() {
    let v = Value::Float(3.14);
    assert!(v.to_string().contains("3.14"));
    let v = Value::Float(-0.5);
    assert!(v.to_string().contains("-0.5"));
}

#[test]
fn test_display_string() {
    assert_eq!(Value::String("hello".to_string()).to_string(), "'hello'");
    assert_eq!(Value::String("".to_string()).to_string(), "''");
}

#[test]
fn test_display_bool() {
    assert_eq!(Value::Bool(true).to_string(), "true");
    assert_eq!(Value::Bool(false).to_string(), "false");
}

#[test]
fn test_display_null() {
    assert_eq!(Value::Null.to_string(), "null");
}

#[test]
fn test_display_array() {
    let arr = Value::Array(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
    ]);
    assert_eq!(arr.to_string(), "[1, 2, 3]");
}

#[test]
fn test_display_array_empty() {
    let arr = Value::Array(vec![]);
    assert_eq!(arr.to_string(), "[]");
}

#[test]
fn test_display_array_nested() {
    let arr = Value::Array(vec![
        Value::Array(vec![Value::Int(1), Value::Int(2)]),
        Value::Array(vec![Value::Int(3)]),
    ]);
    assert_eq!(arr.to_string(), "[[1, 2], [3]]");
}

#[test]
fn test_display_map() {
    let mut map = IndexMap::new();
    map.insert(Value::Int(1), Value::String("a".to_string()));
    map.insert(Value::Int(2), Value::String("b".to_string()));
    let v = Value::Map(map);
    let s = v.to_string();
    assert!(s.starts_with('{'));
    assert!(s.ends_with('}'));
    assert!(s.contains("1: 'a'"));
    assert!(s.contains("2: 'b'"));
}

#[test]
fn test_display_struct() {
    let mut s = IndexMap::new();
    s.insert("x".to_string(), Value::Int(1));
    s.insert("y".to_string(), Value::Float(2.0));
    let v = Value::Struct(s);
    let d = v.to_string();
    assert!(d.starts_with('{'));
    assert!(d.ends_with('}'));
    assert!(d.contains("x: 1"));
    assert!(d.contains("y: 2"));
}

// === PartialEq ===

#[test]
fn test_eq_int() {
    assert_eq!(Value::Int(1), Value::Int(1));
    assert_ne!(Value::Int(1), Value::Int(2));
}

#[test]
fn test_eq_uint() {
    assert_eq!(Value::Uint(5), Value::Uint(5));
    assert_ne!(Value::Uint(5), Value::Uint(6));
}

#[test]
fn test_eq_float() {
    assert_eq!(Value::Float(1.0), Value::Float(1.0));
    assert_ne!(Value::Float(1.0), Value::Float(2.0));
}

#[test]
fn test_eq_cross_type_none() {
    assert_ne!(Value::Int(1), Value::String("1".to_string()));
    assert_ne!(Value::Bool(true), Value::Int(1));
    assert_ne!(Value::Null, Value::Int(0));
}

#[test]
fn test_eq_array() {
    assert_eq!(
        Value::Array(vec![Value::Int(1), Value::Int(2)]),
        Value::Array(vec![Value::Int(1), Value::Int(2)]),
    );
    assert_ne!(
        Value::Array(vec![Value::Int(1)]),
        Value::Array(vec![Value::Int(2)]),
    );
}

#[test]
fn test_eq_map() {
    let mut m1 = IndexMap::new();
    m1.insert(Value::String("k".to_string()), Value::Int(1));
    let mut m2 = IndexMap::new();
    m2.insert(Value::String("k".to_string()), Value::Int(1));
    assert_eq!(Value::Map(m1), Value::Map(m2));
}

#[test]
fn test_eq_struct_same() {
    let mut s1 = IndexMap::new();
    s1.insert("a".to_string(), Value::Int(1));
    let mut s2 = IndexMap::new();
    s2.insert("a".to_string(), Value::Int(1));
    assert_eq!(Value::Struct(s1), Value::Struct(s2));
}

#[test]
fn test_eq_struct_diff_vals() {
    let mut s1 = IndexMap::new();
    s1.insert("a".to_string(), Value::Int(1));
    let mut s2 = IndexMap::new();
    s2.insert("a".to_string(), Value::Int(2));
    assert_ne!(Value::Struct(s1), Value::Struct(s2));
}

// === PartialOrd ===

#[test]
fn test_cmp_int() {
    assert!(Value::Int(1) < Value::Int(2));
    assert!(Value::Int(2) > Value::Int(1));
    assert_eq!(Value::Int(1).partial_cmp(&Value::Int(1)), Some(std::cmp::Ordering::Equal));
}

#[test]
fn test_cmp_float() {
    assert!(Value::Float(1.0) < Value::Float(2.0));
    assert!(Value::Float(2.0) > Value::Float(1.0));
}

#[test]
fn test_cmp_cross_int_float() {
    assert!(Value::Int(1) < Value::Float(2.0));
    assert!(Value::Float(1.5) > Value::Int(1));
    assert_eq!(Value::Int(1).partial_cmp(&Value::Float(1.0)), Some(std::cmp::Ordering::Equal));
}

#[test]
fn test_cmp_cross_uint_float() {
    assert!(Value::Uint(1) < Value::Float(2.0));
    assert!(Value::Float(2.0) > Value::Uint(1));
}

#[test]
fn test_cmp_cross_int_uint() {
    assert!(Value::Int(1) < Value::Uint(5));
    assert!(Value::Uint(5) > Value::Int(1));
    assert_eq!(Value::Int(1).partial_cmp(&Value::Uint(1)), Some(std::cmp::Ordering::Equal));
}

#[test]
fn test_cmp_string() {
    assert!(Value::String("a".to_string()) < Value::String("b".to_string()));
    assert!(Value::String("z".to_string()) > Value::String("a".to_string()));
}

#[test]
fn test_cmp_incomparable() {
    assert_eq!(Value::Bool(true).partial_cmp(&Value::Bool(false)), None);
    assert_eq!(Value::String("a".to_string()).partial_cmp(&Value::Int(1)), None);
    assert_eq!(Value::Null.partial_cmp(&Value::Null), None);
}

#[test]
fn test_cmp_negative_values() {
    assert!(Value::Int(-5) < Value::Int(-1));
    assert!(Value::Int(-1) > Value::Int(-5));
    assert!(Value::Float(-0.5) < Value::Float(0.5));
}

// === Hash ===

#[test]
fn test_hash_consistency() {
    let v1 = Value::Int(42);
    let v2 = Value::Int(42);
    assert_eq!(hash(&v1), hash(&v2));
}

#[test]
fn test_hash_different_values() {
    let v1 = Value::Int(1);
    let v2 = Value::String("1".to_string());
    assert_ne!(hash(&v1), hash(&v2));
}

#[test]
fn test_hash_array_order_matters() {
    let a1 = Value::Array(vec![Value::Int(1), Value::Int(2)]);
    let a2 = Value::Array(vec![Value::Int(2), Value::Int(1)]);
    assert_ne!(hash(&a1), hash(&a2));
}

// === Serialize / Deserialize roundtrip ===

// Note: JSON numbers deserialize to i64/u64/f64 based on Rust type inference.
// Int→Uint is expected since JSON doesn't preserve signedness.
#[test]
fn test_serde_int_to_json() {
    let v = Value::Int(42);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "42");
}

#[test]
fn test_serde_roundtrip_uint() {
    let v = Value::Uint(100);
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn test_serde_roundtrip_float() {
    let v = Value::Float(3.14);
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn test_serde_roundtrip_string() {
    let v = Value::String("hello world".to_string());
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn test_serde_roundtrip_bool() {
    let v = Value::Bool(true);
    let json = serde_json::to_string(&v).unwrap();
    let back: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}

#[test]
fn test_serde_null_serializes() {
    let v = Value::Null;
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "null");
}

#[test]
fn test_serde_array_to_json() {
    let v = Value::Array(vec![
        Value::Int(1),
        Value::String("hello".to_string()),
        Value::Bool(false),
    ]);
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.starts_with('['));
    assert!(json.contains("hello"));
}

#[test]
fn test_serde_struct_to_json() {
    let mut s = IndexMap::new();
    s.insert("id".to_string(), Value::Int(1));
    s.insert("name".to_string(), Value::String("Alice".to_string()));
    let v = Value::Struct(s);
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.contains("\"id\""));
    assert!(json.contains("\"name\""));
    assert!(json.contains("\"Alice\""));
}

#[test]
fn test_serde_map_to_json() {
    let mut m = IndexMap::new();
    m.insert(Value::Int(1), Value::String("one".to_string()));
    m.insert(Value::Int(2), Value::String("two".to_string()));
    let v = Value::Map(m);
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.contains("\"1\""));
    assert!(json.contains("\"one\""));
}

#[test]
fn test_serde_nested_array_to_json() {
    let v = Value::Array(vec![
        Value::Array(vec![Value::Int(1), Value::Int(2)]),
        Value::Array(vec![Value::Int(3), Value::Int(4)]),
    ]);
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.contains("[[1,2],[3,4]]") || json.contains("[[1, 2], [3, 4]]"));
}

#[test]
fn test_serde_empty_array_to_json() {
    let v = Value::Array(vec![]);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn test_serde_empty_struct_to_json() {
    let v = Value::Struct(IndexMap::new());
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_serde_empty_map_to_json() {
    let v = Value::Map(IndexMap::new());
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "{}");
}

// === Serialize to JSON semantics ===

#[test]
fn test_serialize_int_as_json_number() {
    let v = Value::Int(42);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "42");
}

#[test]
fn test_serialize_string_as_json_string() {
    let v = Value::String("hello".to_string());
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "\"hello\"");
}

#[test]
fn test_serialize_bool_as_json_bool() {
    let v = Value::Bool(true);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "true");
}

#[test]
fn test_serialize_null_as_json_null() {
    let v = Value::Null;
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "null");
}

#[test]
fn test_serialize_array_as_json_array() {
    let v = Value::Array(vec![Value::Int(1), Value::Int(2)]);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, "[1,2]");
}

#[test]
fn test_serialize_struct_as_json_object() {
    let mut s = IndexMap::new();
    s.insert("a".to_string(), Value::Int(1));
    s.insert("b".to_string(), Value::String("x".to_string()));
    let v = Value::Struct(s);
    let json = serde_json::to_string(&v).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["a"], 1);
    assert_eq!(parsed["b"], "x");
}

#[test]
fn test_serialize_map_keys_as_strings() {
    let mut m = IndexMap::new();
    m.insert(Value::Int(1), Value::String("one".to_string()));
    m.insert(Value::String("key".to_string()), Value::Int(42));
    let v = Value::Map(m);
    let json = serde_json::to_string(&v).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_object());
    // Map keys are converted to strings in JSON
}

#[test]
fn test_value_clone() {
    let v = Value::Array(vec![Value::Int(1), Value::String("x".to_string())]);
    let v2 = v.clone();
    assert_eq!(v, v2);
}
