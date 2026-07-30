use super::table::constraint::Constraint;
use super::table::field::Field;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schema {
    pub fields: Vec<Field>,
    pub constraints: Vec<Constraint>,
    pub data_start_row: usize,
}

impl Schema {
    /// 兼容旧调用方：fields/constraints 直接给，自动设 data_start_row = 5
    pub fn from_parts(fields: Vec<Field>, constraints: Vec<Constraint>) -> Schema {
        Schema {
            fields,
            constraints,
            data_start_row: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::{Field, FieldType};

    fn dummy_field(name: &str) -> Field {
        Field {
            name: name.to_string(),
            t: FieldType::Int32,
            desc: String::new(),
            constraint: None,
            tags: vec![],
        }
    }

    #[test]
    fn schema_from_parts_defaults_data_start_row_to_5() {
        let s = Schema::from_parts(vec![dummy_field("id")], vec![]);
        assert_eq!(s.data_start_row, 5);
    }

    #[test]
    fn schema_struct_literal_sets_data_start_row_explicitly() {
        let s = Schema {
            fields: vec![dummy_field("id")],
            constraints: vec![],
            data_start_row: 8,
        };
        assert_eq!(s.data_start_row, 8);
    }

    #[test]
    fn schema_clone_equals_original() {
        let s = Schema::from_parts(vec![dummy_field("id")], vec![]);
        assert_eq!(s.clone(), s);
    }
}
