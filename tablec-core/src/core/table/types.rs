use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Int8,
    Int16,
    Int32,
    Int64,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Float32,
    Float64,
    String,
    Bool,
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Struct(HashMap<String, Type>),
    Any,
}
