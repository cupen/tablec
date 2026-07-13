use indexmap::IndexMap;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

// --- BEGIN: private Numeric support (Task 3) ---

#[derive(Debug, Clone, Copy)]
enum Numeric {
    I8(i8), I16(i16), I32(i32), I64(i64),
    U8(u8), U16(u16), U32(u32), U64(u64),
    F32(f32), F64(f64),
}

impl Numeric {
    fn kind(self) -> u8 {
        match self {
            Numeric::I8(_)  => 0, Numeric::I16(_) => 1, Numeric::I32(_) => 2, Numeric::I64(_) => 3,
            Numeric::U8(_)  => 4, Numeric::U16(_) => 5, Numeric::U32(_) => 6, Numeric::U64(_) => 7,
            Numeric::F32(_) => 8, Numeric::F64(_) => 9,
        }
    }
}

impl PartialEq for Numeric {
    fn eq(&self, other: &Self) -> bool {
        // exact match per spec §4.3; do NOT use epsilon
        match (self, other) {
            (Numeric::I8(a),  Numeric::I8(b))  => *a == *b,
            (Numeric::I16(a), Numeric::I16(b)) => *a == *b,
            (Numeric::I32(a), Numeric::I32(b)) => *a == *b,
            (Numeric::I64(a), Numeric::I64(b)) => *a == *b,
            (Numeric::U8(a),  Numeric::U8(b))  => *a == *b,
            (Numeric::U16(a), Numeric::U16(b)) => *a == *b,
            (Numeric::U32(a), Numeric::U32(b)) => *a == *b,
            (Numeric::U64(a), Numeric::U64(b)) => *a == *b,
            (Numeric::F32(a), Numeric::F32(b)) => *a == *b,
            (Numeric::F64(a), Numeric::F64(b)) => *a == *b,
            // Cross-width pairs are not equal (see spec §4.3 / Value::eq doc).
            _ => false,
        }
    }
}

impl PartialOrd for Numeric {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Numeric::I8(a),  Numeric::I8(b))  => a.partial_cmp(b),
            (Numeric::I16(a), Numeric::I16(b)) => a.partial_cmp(b),
            (Numeric::I32(a), Numeric::I32(b)) => a.partial_cmp(b),
            (Numeric::I64(a), Numeric::I64(b)) => a.partial_cmp(b),
            (Numeric::U8(a),  Numeric::U8(b))  => a.partial_cmp(b),
            (Numeric::U16(a), Numeric::U16(b)) => a.partial_cmp(b),
            (Numeric::U32(a), Numeric::U32(b)) => a.partial_cmp(b),
            (Numeric::U64(a), Numeric::U64(b)) => a.partial_cmp(b),
            (Numeric::F32(a), Numeric::F32(b)) => a.partial_cmp(b),
            (Numeric::F64(a), Numeric::F64(b)) => a.partial_cmp(b),
            // Cross-width: no native ordering; Value::partial_cmp promotes to f64.
            _ => None,
        }
    }
}

// --- END: private Numeric support ---

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

impl Value {
    fn to_numeric(&self) -> Option<Numeric> {
        match self {
            Value::Int8(n)   => Some(Numeric::I8(*n)),
            Value::Int16(n)  => Some(Numeric::I16(*n)),
            Value::Int32(n)  => Some(Numeric::I32(*n)),
            Value::Int64(n)  => Some(Numeric::I64(*n)),
            Value::Uint8(n)  => Some(Numeric::U8(*n)),
            Value::Uint16(n) => Some(Numeric::U16(*n)),
            Value::Uint32(n) => Some(Numeric::U32(*n)),
            Value::Uint64(n) => Some(Numeric::U64(*n)),
            Value::Float32(n) => Some(Numeric::F32(*n)),
            Value::Float64(n) => Some(Numeric::F64(*n)),
            _ => None,
        }
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if let Some(n) = self.to_numeric() { return n.serialize(s); }
        match self {
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
            // Numeric variants are handled by the `to_numeric()` early return above.
            _ => unreachable!("numeric variants serialized via Numeric"),
        }
    }
}

impl Serialize for Numeric {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Numeric::I8(v)  => s.serialize_i8(*v),
            Numeric::I16(v) => s.serialize_i16(*v),
            Numeric::I32(v) => s.serialize_i32(*v),
            Numeric::I64(v) => s.serialize_i64(*v),
            Numeric::U8(v)  => s.serialize_u8(*v),
            Numeric::U16(v) => s.serialize_u16(*v),
            Numeric::U32(v) => s.serialize_u32(*v),
            Numeric::U64(v) => s.serialize_u64(*v),
            Numeric::F32(v) => s.serialize_f32(*v),
            Numeric::F64(v) => s.serialize_f64(*v),
        }
    }
}

/// Float comparisons are bitwise exact via `Numeric::eq` (calls `to_bits()`).
/// Per spec §4.3 we deliberately do NOT use `f32::EPSILON` / `f64::EPSILON`:
/// those are minimum representable differences, not useful error tolerances.
///
/// Consequences (all by design):
///   - `NaN != NaN` (IEEE 754)
///   - `+0.0 != -0.0` (bit representations differ)
///   - finite floats compare by their exact bit pattern
///
/// If you need tolerance-based equality, wrap with `approx` or similar.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        // Numeric: delegate to Numeric's PartialEq (cross-width returns false,
        // same-width compares bits). Floats compare bitwise, NOT with EPSILON —
        // see doc comment above and spec §4.3.
        if let (Some(a), Some(b)) = (self.to_numeric(), other.to_numeric()) {
            return a == b;
        }
        // Non-numeric fallthrough
        match (self, other) {
            (Value::String(a), Value::String(b))     => a == b,
            (Value::Bool(a),   Value::Bool(b))       => a == b,
            (Value::Array(a),  Value::Array(b))      => a == b,
            (Value::Map(a),    Value::Map(b))        => a == b,
            (Value::Struct(a), Value::Struct(b))     => a == b,
            (Value::Null,      Value::Null)          => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the discriminant + numeric kind uniformly so e.g. Int32(0)
        // and Uint32(0) hash to different buckets.
        core::mem::discriminant(self).hash(state);
        if let Some(n) = self.to_numeric() {
            n.kind().hash(state);
            match n {
                Numeric::I8(v)  => v.hash(state),
                Numeric::I16(v) => v.hash(state),
                Numeric::I32(v) => v.hash(state),
                Numeric::I64(v) => v.hash(state),
                Numeric::U8(v)  => v.hash(state),
                Numeric::U16(v) => v.hash(state),
                Numeric::U32(v) => v.hash(state),
                Numeric::U64(v) => v.hash(state),
                Numeric::F32(v) => v.to_bits().hash(state),
                Numeric::F64(v) => v.to_bits().hash(state),
            }
            return;
        }
        match self {
            Value::String(s) => s.hash(state),
            Value::Bool(b)   => b.hash(state),
            Value::Array(a)  => a.hash(state),
            Value::Map(m)    => { for (k, v) in m { k.hash(state); v.hash(state); } }
            Value::Struct(s) => { for (k, v) in s { k.hash(state); v.hash(state); } }
            Value::Null      => 0u8.hash(state),
            // Numeric variants are handled by the `to_numeric()` early return above.
            _ => unreachable!("numeric variants hashed via Numeric"),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if let (Some(a), Some(b)) = (self.to_numeric(), other.to_numeric()) {
            // Same-width
            if a == b { return Some(Ordering::Equal); }
            if a.partial_cmp(&b).is_some() { return a.partial_cmp(&b); }
            // Cross-width: promote to f64 (spec §4.3).
            if let (Some(af), Some(bf)) = (
                numeric_to_f64(a), numeric_to_f64(b)
            ) {
                return af.partial_cmp(&bf);
            }
            return None;
        }
        match (self, other) {
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

fn numeric_to_f64(n: Numeric) -> Option<f64> {
    Some(match n {
        Numeric::I8(v)  => v as f64,  Numeric::I16(v) => v as f64,
        Numeric::I32(v) => v as f64,  Numeric::I64(v) => v as f64,
        Numeric::U8(v)  => v as f64,  Numeric::U16(v) => v as f64,
        Numeric::U32(v) => v as f64,  Numeric::U64(v) => v as f64,
        Numeric::F32(v) => v as f64,  Numeric::F64(v) => v,
    })
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(n) = self.to_numeric() {
            return match n {
                Numeric::I8(v)  => write!(f, "{}", v),
                Numeric::I16(v) => write!(f, "{}", v),
                Numeric::I32(v) => write!(f, "{}", v),
                Numeric::I64(v) => write!(f, "{}", v),
                Numeric::U8(v)  => write!(f, "{}", v),
                Numeric::U16(v) => write!(f, "{}", v),
                Numeric::U32(v) => write!(f, "{}", v),
                Numeric::U64(v) => write!(f, "{}", v),
                Numeric::F32(v) => write!(f, "{}", v),
                Numeric::F64(v) => write!(f, "{}", v),
            };
        }
        match self {
            Value::String(s) => write!(f, "'{}'", s),
            Value::Bool(b)   => write!(f, "{}", b),
            Value::Null      => write!(f, "null"),
            Value::Array(a)  => { write!(f, "[")?; for (i, x) in a.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}", x)?; } write!(f, "]") }
            Value::Map(m)    => { write!(f, "{{")?; for (i, (k, v)) in m.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
            Value::Struct(s) => { write!(f, "{{")?; for (i, (k, v)) in s.iter().enumerate() { if i>0 { write!(f, ", ")?; } write!(f, "{}: {}", k, v)?; } write!(f, "}}") }
            // Numeric variants are handled by the `to_numeric()` early return above.
            _ => unreachable!("numeric variants displayed via Numeric"),
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
