use indexmap::IndexMap;
use serde::Serialize;

use super::value::Value;

#[derive(Debug, Serialize)]
pub struct Row {
    #[serde(flatten)]
    pub fields: IndexMap<String, Value>,
}

impl Row {
    pub fn new() -> Self {
        Self { fields: IndexMap::new() }
    }

    pub fn add_field(&mut self, name: String, value: Value) {
        self.fields.insert(name, value);
    }

    pub fn from_vec(fields: Vec<(String, Value)>) -> Self {
        let mut row = Self::new();
        for (name, value) in fields {
            row.add_field(name, value);
        }
        row
    }
}
