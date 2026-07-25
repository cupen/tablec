use super::constraint::Constraint;
use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};
use crate::core::parser::tokenizer::{Token, TokenType, scan_tokens};
use crate::core::table::types::Type;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
    Array {
        r#type: Box<FieldType>,
    },
    Map {
        key: Box<FieldType>,
        value: Box<FieldType>,
    },
    Struct {
        fields: Vec<Field>,
    },
}

impl FieldType {
    pub fn to_type(&self) -> Type {
        match self {
            FieldType::Int8 => Type::Int8,
            FieldType::Int16 => Type::Int16,
            FieldType::Int32 => Type::Int32,
            FieldType::Int64 => Type::Int64,
            FieldType::Int => Type::Int32,
            FieldType::Uint8 => Type::Uint8,
            FieldType::Uint16 => Type::Uint16,
            FieldType::Uint32 => Type::Uint32,
            FieldType::Uint64 => Type::Uint64,
            FieldType::Uint => Type::Uint32,
            FieldType::Float32 => Type::Float32,
            FieldType::Float64 => Type::Float64,
            FieldType::Float => Type::Float32,
            FieldType::String => Type::String,
            FieldType::Bool => Type::Bool,
            FieldType::Date
            | FieldType::DateTime
            | FieldType::Timestamp32
            | FieldType::Timestamp64 => Type::String,
            FieldType::Array { r#type } => Type::Array(Box::new(r#type.to_type())),
            FieldType::Map { key, value } => {
                Type::Map(Box::new(key.to_type()), Box::new(value.to_type()))
            }
            FieldType::Struct { fields } => {
                let mut m = std::collections::HashMap::new();
                for f in fields {
                    m.insert(f.name.clone(), f.t.to_type());
                }
                Type::Struct(m)
            }
        }
    }
}

pub fn parse_field_type(s: &str, loc: SourceLocation) -> Result<FieldType, Diagnostic> {
    let tokens = scan_tokens(s, loc.clone())?;
    let mut peekable = tokens.into_iter().peekable();
    let field_type = parse_from_tokens(&mut peekable, &loc)?;
    if peekable.next().is_some() {
        return Err(Diagnostic::new(
            DiagnosticCode::TypeParseError,
            format!("Extra tokens at end of type definition: {}", s),
            loc,
        ));
    }
    Ok(field_type)
}

impl FromStr for FieldType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_field_type(s, SourceLocation::default())
            .map_err(|d| format!("{}: {}", d.code as u32, d.message))
    }
}

fn parse_from_tokens(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    let mut base_type = parse_base_type(tokens, loc)?;

    // Handle array shorthand `[]`
    while let Some(token) = tokens.peek() {
        if token.value == "[" {
            tokens.next(); // consume '['
            if let Some(next_token) = tokens.next() {
                if next_token.value != "]" {
                    return Err(Diagnostic::new(
                        DiagnosticCode::TypeParseError,
                        "Expected ']' after '['".to_string(),
                        loc.clone(),
                    ));
                }
                base_type = FieldType::Array {
                    r#type: Box::new(base_type),
                };
            } else {
                return Err(Diagnostic::new(
                    DiagnosticCode::TypeParseError,
                    "Expected ']' but found end of input".to_string(),
                    loc.clone(),
                ));
            }
        } else {
            break;
        }
    }
    Ok(base_type)
}

fn parse_base_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    let token = tokens.next().ok_or_else(|| {
        Diagnostic::new(
            DiagnosticCode::TypeParseError,
            "Unexpected end of input".to_string(),
            loc.clone(),
        )
    })?;

    match token.value {
        "array" => parse_array_type(tokens, loc),
        "map" => parse_map_type(tokens, loc),
        "struct" => parse_struct_type(tokens, loc),
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
        _ => Err(Diagnostic::new(
            DiagnosticCode::TypeUnknown,
            format!("Unknown type: {}", token.value),
            loc.clone(),
        )),
    }
}

fn parse_array_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    consume_token(tokens, "<", loc)?;
    let inner_type = parse_from_tokens(tokens, loc)?;
    consume_token(tokens, ">", loc)?;
    Ok(FieldType::Array {
        r#type: Box::new(inner_type),
    })
}

fn parse_map_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    consume_token(tokens, "<", loc)?;
    let key_type = parse_from_tokens(tokens, loc)?;
    consume_token(tokens, ",", loc)?;
    let value_type = parse_from_tokens(tokens, loc)?;
    consume_token(tokens, ">", loc)?;
    Ok(FieldType::Map {
        key: Box::new(key_type),
        value: Box::new(value_type),
    })
}

fn parse_struct_type(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    loc: &SourceLocation,
) -> Result<FieldType, Diagnostic> {
    consume_token(tokens, "{", loc)?;
    let mut fields = Vec::new();

    // Check for empty struct
    if let Some(token) = tokens.peek() {
        if token.value == "}" {
            tokens.next(); // consume '}'
            return Ok(FieldType::Struct { fields });
        }
    }

    loop {
        let name_token = tokens.next().ok_or_else(|| {
            Diagnostic::new(
                DiagnosticCode::TypeParseError,
                "Expected field name".to_string(),
                loc.clone(),
            )
        })?;
        if !matches!(name_token.token_type, TokenType::Word) {
            return Err(Diagnostic::new(
                DiagnosticCode::TypeParseError,
                "Expected a word for field name".to_string(),
                loc.clone(),
            ));
        }

        let field_type = if let Some(token) = tokens.peek() {
            if token.value == ":" {
                tokens.next(); // consume ':'
                parse_from_tokens(tokens, loc)?
            } else {
                FieldType::String // Default type
            }
        } else {
            return Err(Diagnostic::new(
                DiagnosticCode::TypeParseError,
                "Unexpected end of input after field name".to_string(),
                loc.clone(),
            ));
        };

        fields.push(Field {
            name: name_token.value.to_string(),
            t: field_type,
            desc: "".to_string(),
            constraint: None,
            tags: Vec::new(),
        });

        let next_token = tokens.next().ok_or_else(|| {
            Diagnostic::new(
                DiagnosticCode::TypeParseError,
                "Expected '}' or ',' after field".to_string(),
                loc.clone(),
            )
        })?;
        match next_token.value {
            "}" => break,
            "," => continue,
            _ => {
                return Err(Diagnostic::new(
                    DiagnosticCode::TypeParseError,
                    format!("Expected '}}' or ',' but got {}", next_token.value),
                    loc.clone(),
                ));
            }
        }
    }

    Ok(FieldType::Struct { fields })
}

fn consume_token(
    tokens: &mut std::iter::Peekable<std::vec::IntoIter<Token>>,
    expected: &str,
    loc: &SourceLocation,
) -> Result<(), Diagnostic> {
    let token = tokens.next().ok_or_else(|| {
        Diagnostic::new(
            DiagnosticCode::TypeParseError,
            format!("Expected '{}' but found end of input", expected),
            loc.clone(),
        )
    })?;
    if token.value != expected {
        Err(Diagnostic::new(
            DiagnosticCode::TypeParseError,
            format!("Expected '{}' but found '{}'", expected, token.value),
            loc.clone(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_type_preserves_width() {
        assert_eq!(FieldType::Int8.to_type(), Type::Int8);
        assert_eq!(FieldType::Uint16.to_type(), Type::Uint16);
        assert_eq!(FieldType::Float64.to_type(), Type::Float64);
    }

    #[test]
    fn test_parse_simple_types() {
        assert_eq!(FieldType::from_str("int").unwrap(), FieldType::Int32);
        assert_eq!(
            FieldType::from_str("string[]").unwrap(),
            FieldType::Array {
                r#type: Box::new(FieldType::String)
            }
        );
        assert_eq!(
            FieldType::from_str("array<int>").unwrap(),
            FieldType::Array {
                r#type: Box::new(FieldType::Int32)
            }
        );
    }

    #[test]
    fn test_parse_nested_array() {
        let expected = FieldType::Array {
            r#type: Box::new(FieldType::Array {
                r#type: Box::new(FieldType::Int32),
            }),
        };
        assert_eq!(FieldType::from_str("int[][]").unwrap(), expected);
        assert_eq!(FieldType::from_str("array<array<int>>").unwrap(), expected);
    }

    #[test]
    fn test_parse_map_with_complex_types() {
        let expected = FieldType::Map {
            key: Box::new(FieldType::String),
            value: Box::new(FieldType::Array {
                r#type: Box::new(FieldType::Int32),
            }),
        };
        assert_eq!(FieldType::from_str("map<string, int[]>").unwrap(), expected);
        assert_eq!(
            FieldType::from_str("map<string, array<int>>").unwrap(),
            expected
        );
    }

    #[test]
    fn test_parse_struct() {
        let expected = FieldType::Struct {
            fields: vec![
                Field {
                    name: "a".to_string(),
                    t: FieldType::Int32,
                    desc: "".to_string(),
                    constraint: None,
                    tags: vec![],
                },
                Field {
                    name: "b".to_string(),
                    t: FieldType::Array {
                        r#type: Box::new(FieldType::String),
                    },
                    desc: "".to_string(),
                    constraint: None,
                    tags: vec![],
                },
            ],
        };
        assert_eq!(
            FieldType::from_str("struct{a:int, b:string[]}").unwrap(),
            expected
        );
    }
}

#[cfg(test)]
mod parse_field_type_tests {
    use super::*;
    use crate::core::diagnostic::*;

    #[test]
    fn parse_field_type_ok() {
        let ty = parse_field_type("int[]", SourceLocation::default()).unwrap();
        assert_eq!(
            ty,
            FieldType::Array {
                r#type: Box::new(FieldType::Int32)
            }
        );
    }

    #[test]
    fn parse_field_type_bad_returns_diagnostic() {
        let loc = SourceLocation {
            sheet: Some("S".into()),
            line: Some(2),
            ..Default::default()
        };
        let err = parse_field_type("foo", loc.clone()).unwrap_err();
        assert_eq!(err.code, DiagnosticCode::TypeUnknown);
        assert!(err.message.contains("foo"));
        assert_eq!(err.location.sheet, Some("S".into()));
    }
}
