use serde::{Serialize, Deserialize};
use std::str::FromStr;
use crate::core::parser::tokenizer::{scan_tokens, Token, TokenType};
use super::constraint::Constraint;
use std::vec::IntoIter;
use std::iter::Peekable;
use crate::core::table::types::Type;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Field {
    pub name: String,
    pub t: FieldType,
    pub desc: String,
    pub constraint: Option<Constraint>,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum FieldType {
    Int,
    Int8,
    Int16,
    Int32,
    Int64,
    Uint,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Float,
    Float32,
    Float64,
    String,
    Bool,
    Date,
    DateTime,
    Timestamp64,
    Timestamp32,
    Array { r#type: Box<FieldType> },
    Map { key: Box<FieldType>, value: Box<FieldType> },
    Struct { fields: Vec<Field> },
}

impl FieldType {
    pub fn to_type(&self) -> Type {
        match self {
            FieldType::Int | FieldType::Int8 | FieldType::Int16 | FieldType::Int32 | FieldType::Int64 => Type::Int,
            FieldType::Uint | FieldType::Uint8 | FieldType::Uint16 | FieldType::Uint32 | FieldType::Uint64 => Type::Uint,
            FieldType::Float | FieldType::Float32 | FieldType::Float64 => Type::Float,
            FieldType::String => Type::String,
            FieldType::Bool => Type::Bool,
            FieldType::Date | FieldType::DateTime | FieldType::Timestamp32 | FieldType::Timestamp64 => Type::String, // Treat date/time as string for now
            FieldType::Array { r#type } => Type::Array(Box::new(r#type.to_type())),
            FieldType::Map { key, value } => Type::Map(Box::new(key.to_type()), Box::new(value.to_type())),
            FieldType::Struct { fields } => {
                let mut struct_fields = std::collections::HashMap::new();
                for field in fields {
                    struct_fields.insert(field.name.clone(), field.t.to_type());
                }
                Type::Struct(struct_fields)
            }
        }
    }
}

impl FromStr for FieldType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens = scan_tokens(s);
        let mut peekable = tokens.into_iter().peekable();
        let field_type = parse_from_tokens(&mut peekable)?;
        if peekable.next().is_some() {
            Err("Extra tokens found at the end of type definition".to_string())
        } else {
            Ok(field_type)
        }
    }
}

fn parse_from_tokens(tokens: &mut Peekable<IntoIter<Token>>) -> Result<FieldType, String> {
    let mut base_type = parse_base_type(tokens)?;

    // Handle array shorthand `[]`
    while let Some(token) = tokens.peek() {
        if token.value == "[" {
            tokens.next(); // consume '['
            if let Some(next_token) = tokens.next() {
                if next_token.value != "]" {
                    return Err("Expected ']' after '[' for array shorthand".to_string());
                }
                base_type = FieldType::Array { r#type: Box::new(base_type) };
            } else {
                return Err("Expected ']' but found end of input".to_string());
            }
        } else {
            break;
        }
    }
    Ok(base_type)
}

fn parse_base_type(tokens: &mut Peekable<IntoIter<Token>>) -> Result<FieldType, String> {
    let token = tokens.next().ok_or("Unexpected end of input")?;

    match token.value {
        "array" => parse_array_type(tokens),
        "map" => parse_map_type(tokens),
        "struct" => parse_struct_type(tokens),
        "int" => Ok(FieldType::Int32),
        "int8" => Ok(FieldType::Int8),
        "int16" => Ok(FieldType::Int16),
        "int32" => Ok(FieldType::Int32),
        "int64" => Ok(FieldType::Int64),
        "uint" => Ok(FieldType::Uint32),
        "uint8" => Ok(FieldType::Uint8),
        "uint16" => Ok(FieldType::Uint16),
        "uint32" => Ok(FieldType::Uint32),
        "uint64" => Ok(FieldType::Uint64),
        "float" | "float32" => Ok(FieldType::Float32),
        "float64" => Ok(FieldType::Float64),
        "string" | "str" => Ok(FieldType::String),
        "bool" | "boolean" => Ok(FieldType::Bool),
        "date" => Ok(FieldType::Date),
        "datetime" => Ok(FieldType::DateTime),
        "timestamp64" => Ok(FieldType::Timestamp64),
        "timestamp32" => Ok(FieldType::Timestamp32),
        _ => Err(format!("Unknown type: {}", token.value)),
    }
}

fn parse_array_type(tokens: &mut Peekable<IntoIter<Token>>) -> Result<FieldType, String> {
    consume_token(tokens, "<")?;
    let inner_type = parse_from_tokens(tokens)?;
    consume_token(tokens, ">")?;
    Ok(FieldType::Array { r#type: Box::new(inner_type) })
}

fn parse_map_type(tokens: &mut Peekable<IntoIter<Token>>) -> Result<FieldType, String> {
    consume_token(tokens, "<")?;
    let key_type = parse_from_tokens(tokens)?;
    consume_token(tokens, ",")?;
    let value_type = parse_from_tokens(tokens)?;
    consume_token(tokens, ">")?;
    Ok(FieldType::Map { key: Box::new(key_type), value: Box::new(value_type) })
}

fn parse_struct_type(tokens: &mut Peekable<IntoIter<Token>>) -> Result<FieldType, String> {
    consume_token(tokens, "{")?;
    let mut fields = Vec::new();

    // Check for empty struct
    if let Some(token) = tokens.peek() {
        if token.value == "}" {
            tokens.next(); // consume '}'
            return Ok(FieldType::Struct { fields });
        }
    }

    loop {
        let name_token = tokens.next().ok_or("Expected field name")?;
        if !matches!(name_token.token_type, TokenType::Word) {
            return Err("Expected a word for field name".to_string());
        }

        let field_type = if let Some(token) = tokens.peek() {
            if token.value == ":" {
                tokens.next(); // consume ':'
                parse_from_tokens(tokens)?
            } else {
                FieldType::String // Default type
            }
        } else {
            return Err("Unexpected end of input after field name".to_string());
        };

        fields.push(Field {
            name: name_token.value.to_string(),
            t: field_type,
            desc: "".to_string(),
            constraint: None,
            tags: Vec::new(),
        });

        let next_token = tokens.next().ok_or("Expected '}' or ',' after field")?;
        match next_token.value {
            "}" => break,
            "," => continue,
            _ => return Err(format!("Expected '}}' or ',' but got {}", next_token.value)),
        }
    }

    Ok(FieldType::Struct { fields })
}

fn consume_token(tokens: &mut Peekable<IntoIter<Token>>, expected: &str) -> Result<(), String> {
    let token = tokens.next().ok_or(format!("Expected '{}' but found end of input", expected))?;
    if token.value != expected {
        Err(format!("Expected '{}' but found '{}'", expected, token.value))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_types() {
        assert_eq!(FieldType::from_str("int").unwrap(), FieldType::Int32);
        assert_eq!(FieldType::from_str("string[]").unwrap(), FieldType::Array { r#type: Box::new(FieldType::String) });
        assert_eq!(FieldType::from_str("array<int>").unwrap(), FieldType::Array { r#type: Box::new(FieldType::Int32) });
    }

    #[test]
    fn test_parse_nested_array() {
        let expected = FieldType::Array {
            r#type: Box::new(FieldType::Array { r#type: Box::new(FieldType::Int32) })
        };
        assert_eq!(FieldType::from_str("int[][]").unwrap(), expected);
        assert_eq!(FieldType::from_str("array<array<int>>").unwrap(), expected);
    }

    #[test]
    fn test_parse_map_with_complex_types() {
        let expected = FieldType::Map {
            key: Box::new(FieldType::String),
            value: Box::new(FieldType::Array { r#type: Box::new(FieldType::Int32) })
        };
        assert_eq!(FieldType::from_str("map<string, int[]>").unwrap(), expected);
        assert_eq!(FieldType::from_str("map<string, array<int>>").unwrap(), expected);
    }

    #[test]
    fn test_parse_struct() {
        let expected = FieldType::Struct {
            fields: vec![
                Field { name: "a".to_string(), t: FieldType::Int32, desc: "".to_string(), constraint: None, tags: vec![] },
                Field { name: "b".to_string(), t: FieldType::Array { r#type: Box::new(FieldType::String) }, desc: "".to_string(), constraint: None, tags: vec![] },
            ]
        };
        assert_eq!(FieldType::from_str("struct{a:int, b:string[]}").unwrap(), expected);
    }
}