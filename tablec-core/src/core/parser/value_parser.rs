use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};
use crate::core::table::field::FieldType;
use crate::core::table::value::Value;
use crate::core::table::value::Value::{
    Int8, Int16, Int32, Int64, Uint8, Uint16, Uint32, Uint64, Float32, Float64, String as StrVariant,
};
use indexmap::IndexMap;

pub fn parse_value(s: &str, ty: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic> {
    fn parse_basic(s: &str, ty: &FieldType, loc: &SourceLocation) -> Result<Value, Diagnostic> {
        let trimmed = s.trim();
        match ty {
            FieldType::Int8   => trimmed.parse::<i8>().map(Int8).map_err(|_| out_of_range(trimmed, "int8",   -128, 127,   loc)),
            FieldType::Int16  => trimmed.parse::<i16>().map(Int16).map_err(|_| out_of_range(trimmed, "int16",  i16::MIN as i128, i16::MAX as i128, loc)),
            FieldType::Int32  => trimmed.parse::<i32>().map(Int32).map_err(|_| out_of_range(trimmed, "int32",  i32::MIN as i128, i32::MAX as i128, loc)),
            FieldType::Int64  => trimmed.parse::<i64>().map(Int64).map_err(|_| out_of_range(trimmed, "int64",  i64::MIN as i128, i64::MAX as i128, loc)),
            FieldType::Int    => trimmed.parse::<i32>().map(Int32).map_err(|_| out_of_range(trimmed, "int",    i32::MIN as i128, i32::MAX as i128, loc)),
            FieldType::Uint8  => trimmed.parse::<u8>().map(Uint8).map_err(|_| out_of_range(trimmed, "uint8",  0, u8::MAX  as i128, loc)),
            FieldType::Uint16 => trimmed.parse::<u16>().map(Uint16).map_err(|_| out_of_range(trimmed, "uint16", 0, u16::MAX as i128, loc)),
            FieldType::Uint32 => trimmed.parse::<u32>().map(Uint32).map_err(|_| out_of_range(trimmed, "uint32", 0, u32::MAX as i128, loc)),
            FieldType::Uint64 => trimmed.parse::<u64>().map(Uint64).map_err(|_| out_of_range(trimmed, "uint64", 0, u64::MAX as i128, loc)),
            FieldType::Uint   => trimmed.parse::<u32>().map(Uint32).map_err(|_| out_of_range(trimmed, "uint",   0, u32::MAX as i128, loc)),
            FieldType::Float32=> trimmed.parse::<f32>().map(Float32).map_err(|_| parse_fail(trimmed, "float32", loc)),
            FieldType::Float64=> trimmed.parse::<f64>().map(Float64).map_err(|_| parse_fail(trimmed, "float64", loc)),
            FieldType::Float  => trimmed.parse::<f32>().map(Float32).map_err(|_| parse_fail(trimmed, "float", loc)),
            FieldType::Bool   => match trimmed.to_lowercase().as_str() {
                "true" | "1" => Ok(Value::Bool(true)),
                "false" | "0" => Ok(Value::Bool(false)),
                _ => Err(parse_fail(trimmed, "bool", loc)),
            },
            _ => Err(Diagnostic::new(DiagnosticCode::TypeParseError, format!("Unsupported basic type for: {:?}", ty), loc.clone())),
        }
    }

    fn out_of_range(s: &str, ty: &str, lo: i128, hi: i128, loc: &SourceLocation) -> Diagnostic {
        // Distinguish between genuinely out-of-range and non-numeric via a quick attempt.
        if s.trim().parse::<f64>().is_err() {
            parse_fail(s, ty, loc)
        } else {
            Diagnostic::new(
                DiagnosticCode::ValueOutOfRange,
                format!("value '{}' not in {} range [{}, {}]", s, ty, lo, hi),
                loc.clone(),
            )
        }
    }
    fn parse_fail(s: &str, ty: &str, loc: &SourceLocation) -> Diagnostic {
        Diagnostic::new(DiagnosticCode::ValueParseError, format!("cannot parse '{}' as {}", s, ty), loc.clone())
    }

    match ty {
        FieldType::String => {
            let trimmed = s.trim();
            if (trimmed.starts_with('\'') && trimmed.ends_with('\'')) || (trimmed.starts_with('"') && trimmed.ends_with('"')) {
                Ok(StrVariant(trimmed[1..trimmed.len()-1].to_string()))
            } else {
                Ok(StrVariant(trimmed.to_string()))
            }
        }
        FieldType::Date | FieldType::DateTime
            | FieldType::Timestamp32 | FieldType::Timestamp64 => parse_basic(s, &FieldType::String, &loc),
        FieldType::Int8 | FieldType::Int16 | FieldType::Int32 | FieldType::Int64 | FieldType::Int
            | FieldType::Uint8 | FieldType::Uint16 | FieldType::Uint32 | FieldType::Uint64 | FieldType::Uint
            | FieldType::Float32 | FieldType::Float64 | FieldType::Float | FieldType::Bool
            => parse_basic(s, ty, &loc),
        FieldType::Array { r#type } => parse_array(s, r#type, loc),
        FieldType::Map { key, value } => parse_map(s, key, value, loc),
        FieldType::Struct { fields } => parse_struct(s, fields, loc),
    }
}

fn parse_array(s: &str, inner: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic> {
    let t = s.trim();
    if !t.starts_with('[') || !t.ends_with(']') {
        return Err(Diagnostic::new(DiagnosticCode::ValueParseError, "Invalid array format (need [a, b, …])".to_string(), loc));
    }
    let inner_str = &t[1..t.len()-1];
    let mut values = Vec::new();
    let mut level = 0; let mut start = 0;
    for (i, c) in inner_str.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => { values.push(parse_value(inner_str[start..i].trim(), inner, loc.clone())?); start = i + 1; }
            _ => {}
        }
    }
    if start < inner_str.len() {
        values.push(parse_value(inner_str[start..].trim(), inner, loc.clone())?);
    }
    Ok(Value::Array(values))
}

fn parse_map(s: &str, key: &FieldType, value: &FieldType, loc: SourceLocation) -> Result<Value, Diagnostic> {
    let t = s.trim();
    let mut out: IndexMap<Value, Value> = IndexMap::new();
    if t.is_empty() { return Ok(Value::Map(out)); }
    let mut level = 0; let mut start = 0; let mut pairs: Vec<&str> = vec![];
    for (i, c) in t.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => { pairs.push(t[start..i].trim()); start = i + 1; }
            _ => {}
        }
    }
    if start < t.len() { pairs.push(t[start..].trim()); }
    for pair in pairs {
        let parts: Vec<&str> = pair.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(Diagnostic::new(DiagnosticCode::ValueParseError, format!("Invalid map pair: {}", pair), loc.clone()));
        }
        let k = parse_value(parts[0], key, loc.clone())?;
        let v = parse_value(parts[1], value, loc.clone())?;
        out.insert(k, v);
    }
    Ok(Value::Map(out))
}

fn parse_struct(s: &str, fields: &[crate::core::table::field::Field], loc: SourceLocation) -> Result<Value, Diagnostic> {
    let t = s.trim();
    if !t.starts_with('{') || !t.ends_with('}') {
        return Err(Diagnostic::new(DiagnosticCode::ValueParseError, "Invalid struct format (need {a: x, b: y})".to_string(), loc));
    }
    let inner_str = &t[1..t.len()-1];

    // Token-stream by-name: parse each field by walking the comma-split list and matching field names.
    // Use JSON-ish style "name: value" — see spec §4.4 note about struct by-name matching.
    let mut fields_str = Vec::new();
    let mut level = 0; let mut start = 0;
    for (i, c) in inner_str.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => { fields_str.push(&inner_str[start..i]); start = i + 1; }
            _ => {}
        }
    }
    if start < inner_str.len() { fields_str.push(&inner_str[start..]); }

    let mut m: IndexMap<String, Value> = IndexMap::new();
    for chunk in fields_str {
        let chunk = chunk.trim();
        if chunk.is_empty() { continue; }
        let parts: Vec<&str> = chunk.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(Diagnostic::new(DiagnosticCode::StructFieldMismatch,
                format!("expected 'name: value' but got '{}'", chunk), loc.clone()));
        }
        let name = parts[0].trim().to_string();
        let field = match fields.iter().find(|f| f.name == name) {
            Some(f) => f,
            None => return Err(Diagnostic::new(DiagnosticCode::StructFieldMismatch,
                format!("unknown struct field '{}'", name), loc.clone())),
        };
        let v = parse_value(parts[1].trim(), &field.t, loc.clone())?;
        m.insert(name, v);
    }
    // Check all declared fields were seen.
    for f in fields {
        if !m.contains_key(&f.name) {
            return Err(Diagnostic::new(DiagnosticCode::StructFieldCountMismatch,
                format!("struct missing field '{}'", f.name), loc.clone()));
        }
    }
    Ok(Value::Struct(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::{Field, FieldType};

    fn loc() -> SourceLocation { SourceLocation::default() }

    #[test]
    fn parse_int_each_width_ok() {
        assert_eq!(parse_value("42", &FieldType::Int8,  loc()).unwrap(), Value::Int8(42));
        assert_eq!(parse_value("42", &FieldType::Int16, loc()).unwrap(), Value::Int16(42));
        assert_eq!(parse_value("42", &FieldType::Int64, loc()).unwrap(), Value::Int64(42));
        assert_eq!(parse_value("42", &FieldType::Uint8, loc()).unwrap(), Value::Uint8(42));
    }

    #[test]
    fn parse_int_out_of_range_yields_diagnostic() {
        let err = parse_value("200", &FieldType::Int8, loc()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::ValueOutOfRange);
        assert!(err.message.contains("int8"));
        assert!(err.message.contains("["));
    }

    #[test]
    fn parse_non_numeric_yields_parse_error() {
        let err = parse_value("abc", &FieldType::Int32, loc()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::ValueParseError);
    }

    #[test]
    fn parse_struct_by_name_matches_declared_fields() {
        let f1 = Field { name: "a".to_string(), t: FieldType::Int32, desc: "".to_string(), constraint: None, tags: vec![] };
        let f2 = Field { name: "b".to_string(), t: FieldType::String, desc: "".to_string(), constraint: None, tags: vec![] };
        let fields = vec![f1, f2];
        // Order in text doesn't match declaration order, but by-name matches.
        let v = parse_value("{b: hi, a: 7}", &FieldType::Struct { fields: fields.clone() }, loc()).unwrap();
        match v {
            Value::Struct(m) => {
                assert_eq!(m.get("a"), Some(&Value::Int32(7)));
                assert_eq!(m.get("b"), Some(&Value::String("hi".into())));
            }
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn parse_struct_missing_field_reports_count_mismatch() {
        let fields = vec![
            Field { name: "a".to_string(), t: FieldType::Int32, desc: "".to_string(), constraint: None, tags: vec![] },
            Field { name: "b".to_string(), t: FieldType::Int32, desc: "".to_string(), constraint: None, tags: vec![] },
        ];
        let err = parse_value("{a: 1}", &FieldType::Struct { fields }, loc()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::StructFieldCountMismatch);
    }
}
