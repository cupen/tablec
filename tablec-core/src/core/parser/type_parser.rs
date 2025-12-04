use std::collections::HashMap;
use crate::core::table::types::Type;

pub fn parse_type(s: &str) -> Result<Type, String> {
    let s: &str = s.trim();
    if s.ends_with("[]") {
        let inner_type_str = &s[..s.len() - 2];
        let inner_type = parse_type(inner_type_str)?;
        return Ok(Type::Array(Box::new(inner_type)));
    }

    if s.starts_with("map<") && s.ends_with('>') {
        return parse_map_type(s);
    }

    if s.starts_with("struct{") && s.ends_with('}') {
        return parse_struct_type(s);
    }

    match s {
        "int" => Ok(Type::Int),
        "uint" => Ok(Type::Uint),
        "float" => Ok(Type::Float),
        "string" | "str" => Ok(Type::String),
        "bool" => Ok(Type::Bool),
        _ => Err(format!("Unsupported or malformed type string: {}", s)),
    }
}

fn parse_map_type(s: &str) -> Result<Type, String> {
    let inner: &str = &s[4..s.len() - 1];
    let mut balance: i32 = 0;
    let mut comma_pos = None;

    for (i, char) in inner.chars().enumerate() {
        match char {
            '<' | '{' => balance += 1,
            '>' | '}' => balance -= 1,
            ',' if balance == 0 => {
                comma_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    if let Some(pos) = comma_pos {
        let key_str = &inner[..pos];
        let value_str = &inner[pos + 1..];
        let key_type = parse_type(key_str.trim())?;
        let value_type = parse_type(value_str.trim())?;
        Ok(Type::Map(Box::new(key_type), Box::new(value_type)))
    } else {
        Err(format!("Invalid map format: {}", s))
    }
}

fn parse_struct_type(s: &str) -> Result<Type, String> {
    let inner = &s[7..s.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Type::Struct(HashMap::new()));
    }

    let mut fields = HashMap::new();
    let mut balance = 0;
    let mut last_pos = 0;

    for (i, char) in inner.chars().enumerate() {
        match char {
            '<' | '{' => balance += 1,
            '>' | '}' => balance -= 1,
            ',' if balance == 0 => {
                let field_str = &inner[last_pos..i];
                parse_and_add_field(field_str, &mut fields)?;
                last_pos = i + 1;
            }
            _ => {}
        }
    }

    // Parse the last or only field
    parse_and_add_field(&inner[last_pos..], &mut fields)?;

    Ok(Type::Struct(fields))
}

fn parse_and_add_field(field_str: &str, fields: &mut HashMap<String, Type>) -> Result<(), String> {
    let field_str = field_str.trim();
    if field_str.is_empty() {
        return Ok(());
    }

    let parts: Vec<&str> = field_str.splitn(2, ':').collect();
    let name = parts[0].trim();
    if name.is_empty() {
        return Err("Field name cannot be empty in struct".to_string());
    }

    let r#type = if parts.len() == 2 {
        parse_type(parts[1].trim())?
    } else {
        // As per design doc, default to string if type is omitted
        Type::String
    };

    fields.insert(name.to_string(), r#type);
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_types() {
        assert_eq!(parse_type("int").unwrap(), Type::Int);
        assert_eq!(parse_type("string").unwrap(), Type::String);
        assert_eq!(parse_type("str").unwrap(), Type::String);
        assert_eq!(parse_type("bool").unwrap(), Type::Bool);
        assert_eq!(parse_type("float").unwrap(), Type::Float);
    }

    #[test]
    fn test_parse_array_type() {
        assert_eq!(parse_type("int[]").unwrap(), Type::Array(Box::new(Type::Int)));
        assert_eq!(parse_type("string[]").unwrap(), Type::Array(Box::new(Type::String)));
        assert_eq!(parse_type("bool[][]").unwrap(), Type::Array(Box::new(Type::Array(Box::new(Type::Bool)))));
    }

    #[test]
    fn test_parse_map_type() {
        let expected = Type::Map(Box::new(Type::Int), Box::new(Type::String));
        assert_eq!(parse_type("map<int, string>").unwrap(), expected);
    }

    #[test]
    fn test_parse_nested_map_type() {
        let expected = Type::Map(Box::new(Type::String), Box::new(Type::Array(Box::new(Type::Int))));
        assert_eq!(parse_type("map<string, int[]>").unwrap(), expected);
    }

    #[test]
    fn test_parse_simple_struct() {
        let mut fields = HashMap::new();
        fields.insert("a".to_string(), Type::Int);
        fields.insert("b".to_string(), Type::String);
        let expected = Type::Struct(fields);
        assert_eq!(parse_type("struct{a: int, b: string}").unwrap(), expected);
    }

    #[test]
    fn test_parse_struct_with_default_type() {
        let mut fields = HashMap::new();
        fields.insert("a".to_string(), Type::String);
        fields.insert("b".to_string(), Type::Int);
        let expected = Type::Struct(fields);
        assert_eq!(parse_type("struct{a, b: int}").unwrap(), expected);
    }

    #[test]
    fn test_parse_complex_nested_struct() {
        let mut inner_fields = HashMap::new();
        inner_fields.insert("c".to_string(), Type::Float);
        let inner_struct = Type::Struct(inner_fields);

        let mut fields = HashMap::new();
        fields.insert("a".to_string(), Type::Array(Box::new(Type::Int)));
        fields.insert("b".to_string(), Type::Map(Box::new(Type::String), Box::new(inner_struct)));

        let expected = Type::Struct(fields);
        let type_str = "struct{a: int[], b: map<string, struct{c: float}>}";
        assert_eq!(parse_type(type_str).unwrap(), expected);
    }

    #[test]
    fn test_invalid_type() {
        assert!(parse_type("invalid").is_err());
        assert!(parse_type("map<int,").is_err());
        assert!(parse_type("struct{a:int,").is_err());
        assert!(parse_type("struct{a:}").is_err());
    }
}