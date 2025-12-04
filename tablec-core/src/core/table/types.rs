use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    // Basic types
    Int,
    Uint,
    Float,
    String,
    Bool,

    // Complex types
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Struct(HashMap<String, Type>),

    // Special type for untyped or unknown fields
    Any,
}