use crate::core::table::{table::Table, value::Value};
use std::collections::HashSet;

pub fn validate_table(table: &Table) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Validate field-level constraints
    for field in &table.fields {
        if let Some(constraint) = &field.constraint {
            match constraint.func.as_str() {
                "unique" => {
                    if constraint.args.is_empty() {
                        // Single field unique constraint
                        let values: Vec<&Value> = table.data.iter()
                            .map(|row| &row.fields[&field.name])
                            .collect();
                        if let Err(e) = validate_single_unique(&values) {
                            errors.push(format!("Validation failed for field '{}' with constraint '@unique': {}", field.name, e));
                        }
                    } else {
                        // Composite unique constraint
                        let mut composite_fields = vec![field.name.clone()];
                        composite_fields.extend(constraint.args.iter().cloned());
                        if let Err(e) = validate_composite_unique(table, &composite_fields) {
                            errors.push(format!("Validation failed for composite unique constraint on fields {:?}: {}", composite_fields, e));
                        }
                    }
                },
                "order" => {
                    let values: Vec<&Value> = table.data.iter()
                        .map(|row| &row.fields[&field.name])
                        .collect();
                    if let Err(e) = validate_order(&values, &constraint.args) {
                        errors.push(format!("Validation failed for field '{}' with constraint '@order': {}", field.name, e));
                    }
                },
                "seq" => {
                    let values: Vec<&Value> = table.data.iter()
                        .map(|row| &row.fields[&field.name])
                        .collect();
                    if let Err(e) = validate_seq(&values, &constraint.args) {
                        errors.push(format!("Validation failed for field '{}' with constraint '@seq': {}", field.name, e));
                    }
                },
                _ => {},
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_single_unique(values: &[&Value]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for value in values {
        if value != &&Value::Null && !seen.insert(value) {
            return Err(format!("Duplicate value found: {:?}", value));
        }
    }
    Ok(())
}

fn validate_composite_unique(table: &Table, field_names: &[String]) -> Result<(), String> {
    let mut seen_combinations = HashSet::new();

    for row in &table.data {
        let mut combination = Vec::new();
        for field_name in field_names {
            if let Some(value) = row.fields.get(field_name) {
                combination.push(value.clone());
            } else {
                return Err(format!("Field '{}' not found in row for composite unique constraint.", field_name));
            }
        }

        // Treat a combination of all Nulls as unique, or handle as per specific requirements
        if combination.iter().all(|v| matches!(v, Value::Null)) {
            continue; // Or treat as unique, depending on desired behavior for all-null combinations
        }

        if !seen_combinations.insert(combination) {
            return Err(format!("Duplicate combination found for fields {:?}", field_names));
        }
    }
    Ok(())
}

fn validate_order(values: &[&Value], args: &[String]) -> Result<(), String> {
    let desc = args.get(0).map_or(false, |s| s == "desc");
    let mut sorted_values = values.iter().filter(|v| v != &&&Value::Null).cloned().collect::<Vec<_>>();

    // The partial_cmp will only work for types that can be ordered.
    // We rely on the PartialOrd implementation of Value.
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let original_order: Vec<_> = values.iter().filter(|v| v != &&&Value::Null).cloned().collect();

    if desc {
        sorted_values.reverse();
    }

    if original_order != sorted_values {
        return Err("Values are not in the specified order".to_string());
    }

    Ok(())
}

fn validate_seq(values: &[&Value], args: &[String]) -> Result<(), String> {
    let step = args.get(0).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
    let mut expected = 1;

    for (i, value) in values.iter().enumerate() {
        if value == &&Value::Null {
            continue;
        }
        match value {
            Value::Int(v) => {
                if *v != expected {
                    return Err(format!("Sequence mismatch at index {}: expected {}, found {}", i, expected, v));
                }
                expected += step;
            }
            Value::Uint(v) => {
                if *v != expected as u64 {
                    return Err(format!("Sequence mismatch at index {}: expected {}, found {}", i, expected, v));
                }
                expected += step;
            }
            _ => return Err("Sequence validation can only be applied to integer types".to_string()),
        }
    }
    Ok(())
}