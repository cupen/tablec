use crate::core::table::{table::Table, value::Value};
use crate::core::diagnostic::Diagnostic;
use std::collections::HashSet;

pub fn validate_table(table: &Table) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();

    // Validate field-level constraints (row 4).
    for field in &table.fields {
        if let Some(constraint) = &field.constraint {
            let msg = match constraint.func.as_str() {
                "unique" => {
                    if constraint.args.is_empty() {
                        let values: Vec<&Value> = table.data.iter()
                            .map(|row| &row.fields[&field.name])
                            .collect();
                        validate_single_unique(&values).err()
                    } else {
                        let mut composite_fields = vec![field.name.clone()];
                        composite_fields.extend(constraint.args.iter().cloned());
                        validate_composite_unique(table, &composite_fields).err()
                    }
                },
                "order" => {
                    let values: Vec<&Value> = table.data.iter()
                        .map(|row| &row.fields[&field.name])
                        .collect();
                    validate_order(&values, &constraint.args).err()
                },
                "seq" => {
                    let values: Vec<&Value> = table.data.iter()
                        .map(|row| &row.fields[&field.name])
                        .collect();
                    validate_seq(&values, &constraint.args).err()
                },
                _ => None,
            };
            if let Some(m) = msg {
                errors.push(constraint.to_diagnostic(&m));
            }
        }
    }

    // Validate table-level constraints (row 5). Reuse ConstraintValidator's
    // implementation so the table-level logic is not duplicated; field-level
    // constraints are NOT re-run here (already covered above) — we filter.
    for c in &table.constraints {
        match c.validate(&table.fields, &table.data) {
            Ok(()) => {}
            Err(msg) => errors.push(c.to_diagnostic(&msg)),
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
        let n = numeric_i64(value).ok_or_else(|| "Sequence validation can only be applied to integer types".to_string())?;
        if n != expected {
            return Err(format!("Sequence mismatch at index {}: expected {}, found {}", i, expected, n));
        }
        expected += step;
    }
    Ok(())
}

fn numeric_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Int8(n)  => Some(*n as i64),
        Value::Int16(n) => Some(*n as i64),
        Value::Int32(n) => Some(*n as i64),
        Value::Int64(n) => Some(*n),
        Value::Uint8(n)  => Some(*n as i64),
        Value::Uint16(n) => Some(*n as i64),
        Value::Uint32(n) if *n <= i64::MAX as u32 => Some(*n as i64),
        Value::Uint64(n) if *n <= i64::MAX as u64 => Some(*n as i64),
        Value::Uint32(_) | Value::Uint64(_) => None,
        _ => None,
    }
}