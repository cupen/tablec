use serde::{Serialize, Deserialize};
use std::str::FromStr;
use std::collections::HashSet;
use crate::core::table::row::Row;
use crate::core::table::field::Field;
use crate::core::table::value::Value;
use crate::core::table::table::Table;
use crate::core::diagnostic::{Diagnostic, SourceLocation, DiagnosticCode};

use crate::core::table::value::Value::{
    Int8, Int16, Int32, Int64, Uint8, Uint16, Uint32, Uint64,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Constraint {
    pub func: String,
    pub args: Vec<String>,
    pub location: SourceLocation,
}

impl Constraint {
    pub fn from_str_with_loc(s: &str, loc: SourceLocation) -> Result<Self, Diagnostic> {
        if !s.starts_with('@') {
            return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                "constraint must start with @".to_string(), loc));
        }
        let body = &s[1..];
        let (func, args) = if let Some(idx) = body.find('(') {
            // Require a matching ')'. Brief's verbatim `body[idx+1..body.len()-1]`
            // would panic on a missing closing paren; reject explicitly.
            if !body.ends_with(')') {
                return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                    "missing closing parenthesis in constraint".to_string(), loc));
            }
            let f = body[..idx].trim();
            if f.is_empty() {
                return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                    "empty function name".to_string(), loc));
            }
            let arg_str = &body[idx+1..body.len()-1];
            let args: Vec<String> = if arg_str.trim().is_empty() { vec![] } else {
                arg_str.split(',').map(|s| s.trim().to_string()).collect()
            };
            (f.to_string(), args)
        } else {
            // No parens: function name must be a single token (no spaces).
            // Preserves pre-c5 `FromStr` test semantics (`@func arg1, arg2` rejected).
            if body.trim().is_empty() {
                return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                    "empty function name".to_string(), loc));
            }
            let f = body.trim();
            if f.contains(' ') {
                return Err(Diagnostic::new(DiagnosticCode::TableConstraintParseError,
                    "missing parentheses in constraint".to_string(), loc));
            }
            (f.to_string(), vec![])
        };
        Ok(Self { func, args, location: loc })
    }

    pub fn to_diagnostic(&self, msg: &str) -> Diagnostic {
        let code = match self.func.as_str() {
            "unique" => DiagnosticCode::ConstraintDuplicate,
            "seq"    => DiagnosticCode::ConstraintSequenceBroken,
            "order"  => DiagnosticCode::ConstraintOrderViolation,
            _        => DiagnosticCode::ConstraintUnknown,
        };
        let sig = if self.args.is_empty() {
            self.func.clone()
        } else {
            format!("{}({})", self.func, self.args.join(", "))
        };
        Diagnostic::new(code, format!("@{}: {}", sig, msg), self.location.clone())
    }
}

impl FromStr for Constraint {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Constraint::from_str_with_loc(s, SourceLocation::default()).map_err(|_| ())
    }
}

fn numeric_i64(v: &Value) -> Option<i64> {
    match v {
        Int8(n)  => Some(*n as i64),
        Int16(n) => Some(*n as i64),
        Int32(n) => Some(*n as i64),
        Int64(n) => Some(*n),
        Uint8(n)  => Some(*n as i64),
        Uint16(n) => Some(*n as i64),
        // Uint32 may exceed i64 for top bit set; treat as overflow and reject.
        Uint32(n) if *n <= i64::MAX as u32 => Some(*n as i64),
        Uint64(n) if *n <= i64::MAX as u64 => Some(*n as i64),
        Uint32(_) | Uint64(_) => None,
        _ => None,
    }
}

impl Constraint {
    pub fn validate(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        match self.func.as_str() {
            "unique" => self.validate_unique(fields, rows),
            "seq" => self.validate_sequence(fields, rows),
            "order" => self.validate_order(fields, rows),
            _ => Err(format!("Unknown constraint function: {}", self.func)),
        }
    }

    fn validate_unique(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        let field_names: Vec<&str> = if self.args.is_empty() {
            // @unique - validate current field only
            if fields.len() != 1 {
                return Err("@unique constraint requires exactly one field".to_string());
            }
            vec![&fields[0].name]
        } else {
            // @unique(field1, field2) - validate composite uniqueness
            self.args.iter().map(|s| s.as_str()).collect()
        };

        let mut seen_values = HashSet::new();

        for (row_index, row) in rows.iter().enumerate() {
            let mut key_values = Vec::new();

            for field_name in &field_names {
                if let Some(value) = row.get_field(field_name) {
                    key_values.push(value.clone());
                } else {
                    return Err(format!("Field '{}' not found in row {}", field_name, row_index + 1));
                }
            }

            if !seen_values.insert(key_values) {
                return Err(format!("Duplicate values found at row {} for fields: {}",
                    row_index + 1, field_names.join(", ")));
            }
        }

        Ok(())
    }

    fn validate_sequence(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        if fields.len() != 1 {
            return Err("@seq constraint requires exactly one field".to_string());
        }

        let step = if self.args.is_empty() {
            1
        } else {
            self.args[0].parse::<i64>().map_err(|_| "@seq step must be an integer".to_string())?
        };

        let field_name = &fields[0].name;
        let mut expected_value = 1;

        for (row_index, row) in rows.iter().enumerate() {
            if let Some(value) = row.get_field(field_name) {
                let n = numeric_i64(value).ok_or_else(|| format!("@seq requires numeric field '{}'", field_name))?;
                if n != expected_value {
                    return Err(format!("expected {} at row {} but found {}", expected_value, row_index + 1, n));
                }
            } else {
                return Err(format!("Field '{}' not found in row {}", field_name, row_index + 1));
            }

            expected_value += step;
        }

        Ok(())
    }

    fn validate_order(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        if fields.len() != 1 {
            return Err("@order constraint requires exactly one field".to_string());
        }

        let order_type = if self.args.is_empty() {
            "asc"
        } else {
            &self.args[0]
        };

        let field_name = &fields[0].name;
        let mut prev_value: Option<&Value> = None;

        for (row_index, row) in rows.iter().enumerate() {
            if let Some(current_value) = row.get_field(field_name) {
                if let Some(prev) = prev_value {
                    match (order_type, prev.partial_cmp(current_value)) {
                        ("asc", Some(std::cmp::Ordering::Greater)) => {
                            return Err(format!("Order violation at row {}: {} > {} (expected ascending order)",
                                row_index + 1, prev, current_value));
                        }
                        ("desc", Some(std::cmp::Ordering::Less)) => {
                            return Err(format!("Order violation at row {}: {} < {} (expected descending order)",
                                row_index + 1, prev, current_value));
                        }
                        (_, None) => {
                            return Err(format!("Cannot compare values at row {}: {} and {}",
                                row_index + 1, prev, current_value));
                        }
                        _ => {} // Order is correct
                    }
                }
                prev_value = Some(current_value);
            } else {
                return Err(format!("Field '{}' not found in row {}", field_name, row_index + 1));
            }
        }

        Ok(())
    }
}

pub struct ConstraintValidator;

impl ConstraintValidator {
    pub fn validate_table(table: &Table) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();

        // Field-level constraints (constraint declared in row 4 column).
        for field in &table.fields {
            if let Some(constraint) = &field.constraint {
                if let Err(msg) = constraint.validate(&[field.clone()], &table.data) {
                    errors.push(constraint.to_diagnostic(&msg));
                }
            }
        }

        // Table-level constraints (row 5).
        for constraint in &table.constraints {
            if let Err(msg) = constraint.validate(&table.fields, &table.data) {
                errors.push(constraint.to_diagnostic(&msg));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::FieldType;

    #[test]
    fn test_ok() {
        let valid_str = "@func(arg1, arg2)";
        let constraint = Constraint::from_str(valid_str).unwrap();
        assert_eq!(constraint.func, "func");
        assert_eq!(constraint.args, vec!["arg1", "arg2"]);
    }

    #[test]
    fn test_fail() {
        let bad_cases = vec![
            "@func arg1, arg2", // Missing parentheses
            "@func(arg1, arg2", // Missing closing parenthesis
            "func(arg1, arg2)", // Missing @ prefix
            "@",                // Just @ with no function name
        ];

        for _case in bad_cases {
            let rs = Constraint::from_str(_case);
            assert!(rs.is_err(), "Expected error for case: '{_case}'");
        }
    }

    #[test]
    fn test_constraint_validation() {
        use crate::core::table::row::Row;
        use crate::core::table::value::Value;
        use crate::core::table::field::FieldType;

        // Test @unique constraint
        let unique_constraint = Constraint::from_str("@unique").unwrap();
        let fields = vec![Field {
            name: "id".to_string(),
            t: FieldType::Int32,
            desc: "".to_string(),
            constraint: Some(unique_constraint.clone()),
            tags: vec![],
        }];

        let rows = vec![
            Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("id".to_string(), Value::Int32(2))]),
        ];

        assert!(unique_constraint.validate(&fields, &rows).is_ok());

        // Test duplicate values
        let duplicate_rows = vec![
            Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("id".to_string(), Value::Int32(1))]),
        ];

        assert!(unique_constraint.validate(&fields, &duplicate_rows).is_err());

        // Test @seq constraint
        let seq_constraint = Constraint::from_str("@seq").unwrap();
        let seq_fields = vec![Field {
            name: "seq".to_string(),
            t: FieldType::Int32,
            desc: "".to_string(),
            constraint: Some(seq_constraint.clone()),
            tags: vec![],
        }];

        let seq_rows = vec![
            Row::from_vec(vec![("seq".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("seq".to_string(), Value::Int32(2))]),
            Row::from_vec(vec![("seq".to_string(), Value::Int32(3))]),
        ];

        assert!(seq_constraint.validate(&seq_fields, &seq_rows).is_ok());

        // Test broken sequence
        let broken_rows = vec![
            Row::from_vec(vec![("seq".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("seq".to_string(), Value::Int32(3))]),
        ];

        assert!(seq_constraint.validate(&seq_fields, &broken_rows).is_err());

        // Test @order constraint
        let order_constraint = Constraint::from_str("@order").unwrap();
        let order_fields = vec![Field {
            name: "value".to_string(),
            t: FieldType::Int32,
            desc: "".to_string(),
            constraint: Some(order_constraint.clone()),
            tags: vec![],
        }];

        let ordered_rows = vec![
            Row::from_vec(vec![("value".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("value".to_string(), Value::Int32(2))]),
            Row::from_vec(vec![("value".to_string(), Value::Int32(3))]),
        ];

        assert!(order_constraint.validate(&order_fields, &ordered_rows).is_ok());

        // Test unordered rows
        let unordered_rows = vec![
            Row::from_vec(vec![("value".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("value".to_string(), Value::Int32(3))]),
            Row::from_vec(vec![("value".to_string(), Value::Int32(2))]),
        ];

        assert!(order_constraint.validate(&order_fields, &unordered_rows).is_err());
    }

    #[test]
    fn validate_sequence_handles_each_width() {
        let c = Constraint::from_str("@seq").unwrap();
        let fields = vec![Field {
            name: "n".into(), t: FieldType::Int16,
            desc: "".into(), constraint: Some(c.clone()), tags: vec![],
        }];
        let rows = vec![
            Row::from_vec(vec![("n".into(), Value::Int16(1))]),
            Row::from_vec(vec![("n".into(), Value::Int16(2))]),
        ];
        assert!(c.validate(&fields, &rows).is_ok());
    }
}