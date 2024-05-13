
use std::error::Error;
use serde::Serialize;

// Define column types enum
#[derive(Debug, Serialize)]
pub enum Type {
    Int,
    String,
    Float,
    ArrayInt,
    ArrayString,
}

impl Type {
    // Function to parse string into ColumnType
    pub fn from_str(type_str: &str) -> Option<Self> {
        match type_str {
            "int" => Some(Type::Int),
            "string" => Some(Type::String),
            "float" => Some(Type::Float),
            "array<int>" => Some(Type::ArrayInt),
            "array<string>" => Some(Type::ArrayString),
            _ => panic!("Unknown column type: {}", type_str),
        }
    }
}