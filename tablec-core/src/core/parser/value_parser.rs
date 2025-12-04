use crate::core::table::types::Type;
use crate::core::table::value::Value;
use indexmap::IndexMap;

pub fn parse_value(value_str: &str, value_type: &Type) -> Result<Value, String> {
    match value_type {
        Type::Int => parse_basic_value(value_str, &Type::Int),
        Type::Uint => parse_basic_value(value_str, &Type::Uint),
        Type::Float => parse_basic_value(value_str, &Type::Float),
        Type::String => {
            let trimmed = value_str.trim();
            if (trimmed.starts_with('\'') && trimmed.ends_with('\'')) || (trimmed.starts_with('"') && trimmed.ends_with('"')) {
                Ok(Value::String(trimmed[1..trimmed.len() - 1].to_string()))
            } else {
                Ok(Value::String(trimmed.to_string()))
            }
        },
        Type::Bool => parse_basic_value(value_str, &Type::Bool),
        Type::Array(inner_type) => parse_array(value_str, inner_type),
        Type::Map(key_type, value_type) => parse_map(value_str, key_type, value_type),
        Type::Struct(fields) => parse_struct(value_str, fields),
        Type::Any => Ok(Value::String(value_str.to_string())), // Default to string if type is not specified
    }
}

fn parse_basic_value(value_str: &str, value_type: &Type) -> Result<Value, String> {
    let trimmed = value_str.trim();
    match value_type {
        Type::Int => trimmed.parse::<i64>().map(Value::Int).map_err(|e| e.to_string()),
        Type::Uint => trimmed.parse::<u64>().map(Value::Uint).map_err(|e| e.to_string()),
        Type::Float => trimmed.parse::<f64>().map(Value::Float).map_err(|e| e.to_string()),
        Type::Bool => trimmed.parse::<bool>().map(Value::Bool).map_err(|e| e.to_string()),
        _ => Err("Unsupported basic type".to_string()),
    }
}

fn parse_array(value_str: &str, inner_type: &Type) -> Result<Value, String> {
    let trimmed = value_str.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err("Invalid array format".to_string());
    }
    let inner_str = &trimmed[1..trimmed.len() - 1];
    let mut values = Vec::new();
    let mut level = 0;
    let mut start = 0;

    for (i, c) in inner_str.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => {
                values.push(parse_value(inner_str[start..i].trim(), inner_type)?);
                start = i + 1;
            },
            _ => {},
        }
    }
    if start < inner_str.len() {
        values.push(parse_value(inner_str[start..].trim(), inner_type)?);
    }

    Ok(Value::Array(values))
}

fn parse_map(value_str: &str, key_type: &Type, value_type: &Type) -> Result<Value, String> {
    let mut map = IndexMap::new();
    let inner_str = value_str.trim();
    if inner_str.is_empty() {
        return Ok(Value::Map(map));
    }

    let mut level = 0;
    let mut start = 0;
    let mut pairs = Vec::new();

    for (i, c) in inner_str.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => {
                pairs.push(inner_str[start..i].trim());
                start = i + 1;
            },
            _ => {},
        }
    }
    if start < inner_str.len() {
        pairs.push(inner_str[start..].trim());
    }

    for pair in pairs {
        let parts: Vec<&str> = pair.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid map pair: {}", pair));
        }
        let key = parse_value(parts[0], key_type)?;
        let value = parse_value(parts[1], value_type)?;
        map.insert(key, value);
    }
    Ok(Value::Map(map))
}

fn parse_struct(value_str: &str, fields: &std::collections::HashMap<String, Type>) -> Result<Value, String> {
    let mut s = IndexMap::new();
    let trimmed = value_str.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err("Invalid struct format".to_string());
    }
    let inner_str = &trimmed[1..trimmed.len() - 1];

    let mut values_str = Vec::new();
    let mut level = 0;
    let mut start = 0;

    for (i, c) in inner_str.chars().enumerate() {
        match c {
            '[' | '{' | '<' => level += 1,
            ']' | '}' | '>' => level -= 1,
            ',' if level == 0 => {
                values_str.push(inner_str[start..i].trim());
                start = i + 1;
            },
            _ => {},
        }
    }
    if start < inner_str.len() {
        values_str.push(inner_str[start..].trim());
    }

    let field_names: Vec<&String> = fields.keys().collect();

    if values_str.len() != field_names.len() {
        return Err(format!("Struct value count ({}) does not match field count ({})", values_str.len(), field_names.len()));
    }

    for (i, value_str) in values_str.iter().enumerate() {
        let field_name = field_names[i];
        let field_type = &fields[field_name];
        let value = parse_value(value_str, field_type)?;
        s.insert(field_name.clone(), value);
    }

    Ok(Value::Struct(s))
}