use indexmap::IndexMap;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub enum Value {
    Int8(i8), Int16(i16), Int32(i32), Int64(i64),
    Uint8(u8), Uint16(u16), Uint32(u32), Uint64(u64),
    Float32(f32), Float64(f64),
    String(String), Bool(bool),
    Array(Vec<Value>),
    Map(IndexMap<Value, Value>),
    Struct(IndexMap<String, Value>),
    Null,
}

fn numeric_kind(v: &Value) -> Option<u8> {
    match v {
        Value::Int8(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_)
        | Value::Uint8(_) | Value::Uint16(_) | Value::Uint32(_) | Value::Uint64(_) => Some(0),
        Value::Float32(_) | Value::Float64(_) => Some(1),
        _ => None,
    }
}

fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int8(n) => Some(*n as f64),
        Value::Int16(n) => Some(*n as f64),
        Value::Int32(n) => Some(*n as f64),
        Value::Int64(n) => Some(*n as f64),
        Value::Uint8(n) => Some(*n as f64),
        Value::Uint16(n) => Some(*n as f64),
        Value::Uint32(n) => Some(*n as f64),
        Value::Uint64(n) => Some(*n as f64),
        Value::Float32(n) => Some(*n as f64),
        Value::Float64(n) => Some(*n),
        _ => None,
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Int8(n)   => s.serialize_i8(*n),
            Value::Int16(n)  => s.serialize_i16(*n),
            Value::Int32(n)  => s.serialize_i32(*n),
            Value::Int64(n)  => s.serialize_i64(*n),
            Value::Uint8(n)  => s.serialize_u8(*n),
            Value::Uint16(n) => s.serialize_u16(*n),
            Value::Uint32(n) => s.serialize_u32(*n),
            Value::Uint64(n) => s.serialize_u64(*n),
            Value::Float32(n) => s.serialize_f32(*n),
            Value::Float64(n) => s.serialize_f64(*n),
            Value::String(v) => s.serialize_str(v),
            Value::Bool(b)   => s.serialize_bool(*b),
            Value::Array(a)  => a.serialize(s),
            Value::Struct(m) => m.serialize(s),
            Value::Null      => s.serialize_none(),
            Value::Map(m)    => {
                let mut map = s.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    let key_str = match k {
                        Value::String(st) => st.clone(),
                        Value::Int8(n) => n.to_string(),
                        Value::Int16(n) => n.to_string(),
                        Value::Int32(n) => n.to_string(),
                        Value::Int64(n) => n.to_string(),
                        Value::Uint8(n) => n.to_string(),
                        Value::Uint16(n) => n.to_string(),
                        Value::Uint32(n) => n.to_string(),
                        Value::Uint64(n) => n.to_string(),
                        Value::Float32(n) => n.to_string(),
                        Value::Float64(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_string(),
                        _ => return Err(serde::ser::Error::custom("Map keys must be simple types")),
                    };
                    map.serialize_entry(&key_str, v)?;
                }
                map.end()
            }
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        // Same-variant exact compare.
        if core::mem::discriminant(self) != core::mem::discriminant(other) { return false; }
        match (self, other) {
            (Value::Int8(a),   Value::Int8(b))   => a == b,
            (Value::Int16(a),  Value::Int16(b))  => a == b,
            (Value::Int32(a),  Value::Int32(b))  => a == b,
            (Value::Int64(a),  Value::Int64(b))  => a == b,
            (Value::Uint8(a),  Value::Uint8(b))  => a == b,
            (Value::Uint16(a), Value::Uint16(b)) => a == b,
            (Value::Uint32(a), Value::Uint32(b)) => a == b,
            (Value::Uint64(a), Value::Uint64(b)) => a == b,
            (Value::Float32(a), Value::Float32(b)) => (a - b).abs() < f32::EPSILON,
            (Value::Float64(a), Value::Float64(b)) => (a - b).abs() < f64::EPSILON,
            (Value::String(a), Value::String(b))   => a == b,
            (Value::Bool(a),   Value::Bool(b))     => a == b,
            (Value::Array(a),  Value::Array(b))    => a == b,
            (Value::Map(a),    Value::Map(b))      => a == b,
            (Value::Struct(a), Value::Struct(b))   => a == b,
            (Value::Null,      Value::Null)        => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Value::Int8(n)   => n.hash(state),
            Value::Int16(n)  => n.hash(state),
            Value::Int32(n)  => n.hash(state),
            Value::Int64(n)  => n.hash(state),
            Value::Uint8(n)  => n.hash(state),
            Value::Uint16(n) => n.hash(state),
            Value::Uint32(n) => n.hash(state),
            Value::Uint64(n) => n.hash(state),
            Value::Float32(n) => n.to_bits().hash(state),
            Value::Float64(n) => n.to_bits().hash(state),
            Value::String(s) => s.hash(state),
            Value::Bool(b)   => b.hash(state),
            Value::Array(a)  => a.hash(state),
            Value::Map(m)    => { for (k, v) in m { k.hash(state); v.hash(state); } }
            Value::Struct(s) => { for (k, v) in s { k.hash(state); v.hash(state); } }
            Value::Null      => 0u8.hash(state),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Value::*;
        // Same-width numeric comparisons: native ordering.
        match (self, other) {
            (Int8(a),   Int8(b))   => return a.partial_cmp(b),
            (Int16(a),  Int16(b))  => return a.partial_cmp(b),
            (Int32(a),  Int32(b))  => return a.partial_cmp(b),
            (Int64(a),  Int64(b))  => return a.partial_cmp(b),
            (Uint8(a),  Uint8(b))  => return a.partial_cmp(b),
            (Uint16(a), Uint16(b)) => return a.partial_cmp(b),
            (Uint32(a), Uint32(b)) => return a.partial_cmp(b),
            (Uint64(a), Uint64(b)) => return a.partial_cmp(b),
            (Float32(a), Float32(b)) => return a.partial_cmp(b),
            (Float64(a), Float64(b)) => return a.partial_cmp(b),
            (String(a), String(b))   => return a.partial_cmp(b),
            _ => {}
        }
        // Float↔Float cross-width.
        if let (Float32(a), Float64(b)) = (self, other) {
            return (*a as f64).partial_cmp(b);
        }
        if let (Float64(a), Float32(b)) = (self, other) {
            return a.partial_cmp(&(*b as f64));
        }
        // Cross-width or cross-family numeric via f64 widening (parse-time range-checked).
        if numeric_kind(self).is_some() && numeric_kind(other).is_some() {
            match (to_f64(self), to_f64(other)) {
                (Some(av), Some(bv)) => return av.partial_cmp(&bv),
                _ => {}
            }
        }
        None
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int8(n)   => write!(f, "{}", n),
            Value::Int16(n)  => write!(f, "{}", n),
            Value::Int32(n)  => write!(f, "{}", n),
            Value::Int64(n)  => write!(f, "{}", n),
            Value::Uint8(n)  => write!(f, "{}", n),
            Value::Uint16(n) => write!(f, "{}", n),
            Value::Uint32(n) => write!(f, "{}", n),
            Value::Uint64(n) => write!(f, "{}", n),
            Value::Float32(n) => write!(f, "{}", n),
            Value::Float64(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "'{}'", s),
            Value::Bool(b)   => write!(f, "{}", b),
            Value::Null      => write!(f, "null"),
            Value::Array(a)  => { write!(f, "[")?; for (i, x) in a.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}", x)?; } write!(f, "]") }
            Value::Map(m)    => { write!(f, "{{")?; for (i, (k, v)) in m.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
            Value::Struct(s) => { write!(f, "{{")?; for (i, (k, v)) in s.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
        }
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("a valid Value") }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> { Ok(Value::Bool(v)) }
    fn visit_i8<E: de::Error>(self, v: i8) -> Result<Value, E> { Ok(Value::Int8(v)) }
    fn visit_i16<E: de::Error>(self, v: i16) -> Result<Value, E> { Ok(Value::Int16(v)) }
    fn visit_i32<E: de::Error>(self, v: i32) -> Result<Value, E> { Ok(Value::Int32(v)) }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> { Ok(Value::Int64(v)) }
    fn visit_u8<E: de::Error>(self, v: u8) -> Result<Value, E> { Ok(Value::Uint8(v)) }
    fn visit_u16<E: de::Error>(self, v: u16) -> Result<Value, E> { Ok(Value::Uint16(v)) }
    fn visit_u32<E: de::Error>(self, v: u32) -> Result<Value, E> { Ok(Value::Uint32(v)) }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> { Ok(Value::Uint64(v)) }
    fn visit_f32<E: de::Error>(self, v: f32) -> Result<Value, E> { Ok(Value::Float32(v)) }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> { Ok(Value::Float64(v)) }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> { Ok(Value::String(v.to_string())) }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> { Ok(Value::String(v)) }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> { Ok(Value::Null) }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> { d.deserialize_any(self) }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut v = Vec::new();
        while let Some(x) = seq.next_element()? { v.push(x); }
        Ok(Value::Array(v))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Value, A::Error> {
        let mut m = IndexMap::new();
        while let Some((k, v)) = access.next_entry::<String, Value>()? { m.insert(k, v); }
        Ok(Value::Struct(m))
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> { d.deserialize_any(ValueVisitor) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_size_is_sixteen_variants() {
        // Lock the variants; bump ONLY when adding/removing a variant.
        let mut seen = std::collections::HashSet::new();
        seen.insert(std::mem::discriminant(&Value::Int8(0)));
        seen.insert(std::mem::discriminant(&Value::Int16(0)));
        seen.insert(std::mem::discriminant(&Value::Int32(0)));
        seen.insert(std::mem::discriminant(&Value::Int64(0)));
        seen.insert(std::mem::discriminant(&Value::Uint8(0)));
        seen.insert(std::mem::discriminant(&Value::Uint16(0)));
        seen.insert(std::mem::discriminant(&Value::Uint32(0)));
        seen.insert(std::mem::discriminant(&Value::Uint64(0)));
        seen.insert(std::mem::discriminant(&Value::Float32(0.0)));
        seen.insert(std::mem::discriminant(&Value::Float64(0.0)));
        seen.insert(std::mem::discriminant(&Value::String(String::new())));
        seen.insert(std::mem::discriminant(&Value::Bool(false)));
        seen.insert(std::mem::discriminant(&Value::Array(vec![])));
        seen.insert(std::mem::discriminant(&Value::Map(IndexMap::new())));
        seen.insert(std::mem::discriminant(&Value::Struct(IndexMap::new())));
        seen.insert(std::mem::discriminant(&Value::Null));
        assert_eq!(seen.len(), 16);
    }

    #[test]
    fn cross_width_partial_ord_promotes() {
        assert!(Value::Int8(-1) < Value::Uint8(1));
        assert!(Value::Int32(1) < Value::Int64(2));
        // Float vs Int cross-family:
        assert!(Value::Float32(1.5) > Value::Int32(1));
    }

    #[test]
    fn hash_includes_discriminant() {
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        Value::Int32(0).hash(&mut h1);
        Value::Uint32(0).hash(&mut h2);
        assert_ne!(h1.finish(), h2.finish(), "Int32(0) and Uint32(0) must hash differently");
    }

    #[test]
    fn serialize_each_numeric_variant() {
        let cases = vec![
            (Value::Int8(-1),   "-1"),
            (Value::Int16(-1),  "-1"),
            (Value::Int32(-1),  "-1"),
            (Value::Int64(-1),  "-1"),
            (Value::Uint8(1),   "1"),
            (Value::Uint16(1),  "1"),
            (Value::Uint32(1),  "1"),
            (Value::Uint64(1),  "1"),
            (Value::Float32(1.5), "1.5"),
            (Value::Float64(1.5), "1.5"),
        ];
        for (v, expected) in cases {
            let s = serde_json::to_string(&v).unwrap();
            assert_eq!(s, expected, "variant {:?}", v);
        }
    }
}
