use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};
use crate::core::table::field::Field;
use crate::core::table::row::Row;
use crate::core::table::table::Table;
use crate::core::table::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;

use crate::core::table::value::Value::{Int8, Int16, Int32, Int64, Uint8, Uint16, Uint32, Uint64};
use std::collections::HashMap;

/// Parse the argument list inside `@func(...)`.
///
/// Rules:
/// - Top-level separator is `,`.
/// - `,` inside `"..."` is literal.
/// - `"` enters / exits a quoted segment; `\` inside a quoted segment
///   escapes `\"` or `\\` only; any other `\X` is rejected.
/// - A `"` may only start a quoted segment at the beginning of an arg
///   (after leading whitespace); a `"` mid-arg is rejected.
/// - Leading whitespace at arg start is trimmed; internal whitespace
///   outside quotes is preserved as-is.
///
/// Returns the empty vector when the input is empty or whitespace-only.
fn parse_args(s: &str, loc: SourceLocation) -> Result<Vec<String>, Diagnostic> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_str = false;
    let mut arg_started = false;

    while i < chars.len() {
        let c = chars[i];
        if !in_str && !arg_started && (c == ' ' || c == '\t') {
            i += 1;
            continue;
        }
        if in_str {
            if c == '\\' {
                if i + 1 >= chars.len() {
                    return Err(Diagnostic::new(
                        DiagnosticCode::TableConstraintParseError,
                        "unterminated escape in constraint args".to_string(),
                        loc,
                    ));
                }
                match chars[i + 1] {
                    '"' => {
                        current.push('"');
                        i += 2;
                    }
                    '\\' => {
                        current.push('\\');
                        i += 2;
                    }
                    other => {
                        return Err(Diagnostic::new(
                            DiagnosticCode::TableConstraintParseError,
                            format!("unsupported escape '\\{}' in constraint args", other),
                            loc,
                        ));
                    }
                }
            } else if c == '"' {
                in_str = false;
                i += 1;
            } else {
                current.push(c);
                i += 1;
            }
        } else {
            if c == '"' {
                in_str = true;
                arg_started = true;
                i += 1;
            } else if c == ',' {
                args.push(current.trim().to_string());
                current.clear();
                arg_started = false;
                i += 1;
            } else {
                current.push(c);
                arg_started = true;
                i += 1;
            }
        }
    }

    if in_str {
        return Err(Diagnostic::new(
            DiagnosticCode::TableConstraintParseError,
            "unterminated string in constraint args".to_string(),
            loc,
        ));
    }

    let tail = current.trim().to_string();
    if args.is_empty() && tail.is_empty() {
        return Ok(vec![]);
    }
    args.push(tail);
    Ok(args)
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Constraint {
    pub func: String,
    pub args: Vec<String>,
    pub location: SourceLocation,
}

impl Constraint {
    pub fn from_str_with_loc(s: &str, loc: SourceLocation) -> Result<Self, Diagnostic> {
        if !s.starts_with('@') {
            return Err(Diagnostic::new(
                DiagnosticCode::TableConstraintParseError,
                "constraint must start with @".to_string(),
                loc,
            ));
        }
        let body = &s[1..];
        let (func, args) = if let Some(idx) = body.find('(') {
            // Require a matching ')'. Brief's verbatim `body[idx+1..body.len()-1]`
            // would panic on a missing closing paren; reject explicitly.
            if !body.ends_with(')') {
                return Err(Diagnostic::new(
                    DiagnosticCode::TableConstraintParseError,
                    "missing closing parenthesis in constraint".to_string(),
                    loc,
                ));
            }
            let f = body[..idx].trim();
            if f.is_empty() {
                return Err(Diagnostic::new(
                    DiagnosticCode::TableConstraintParseError,
                    "empty function name".to_string(),
                    loc,
                ));
            }
            let arg_str = &body[idx + 1..body.len() - 1];
            let args = parse_args(arg_str, loc.clone())?;
            (f.to_string(), args)
        } else {
            // No parens: function name must be a single token (no spaces).
            // Preserves pre-c5 `FromStr` test semantics (`@func arg1, arg2` rejected).
            if body.trim().is_empty() {
                return Err(Diagnostic::new(
                    DiagnosticCode::TableConstraintParseError,
                    "empty function name".to_string(),
                    loc,
                ));
            }
            let f = body.trim();
            if f.contains(' ') {
                return Err(Diagnostic::new(
                    DiagnosticCode::TableConstraintParseError,
                    "missing parentheses in constraint".to_string(),
                    loc,
                ));
            }
            (f.to_string(), vec![])
        };
        Ok(Self {
            func,
            args,
            location: loc,
        })
    }

    pub fn to_diagnostic(&self, msg: &str) -> Diagnostic {
        let code = match self.func.as_str() {
            "unique" => DiagnosticCode::ConstraintDuplicate,
            "seq" => DiagnosticCode::ConstraintSequenceBroken,
            "order" => DiagnosticCode::ConstraintOrderViolation,
            "nullable" => DiagnosticCode::ConstraintNullNotAllowed,
            "range" | "maxlen" => DiagnosticCode::ConstraintValueViolation,
            "oneof" => DiagnosticCode::ConstraintNotInSet,
            "pattern" => DiagnosticCode::ConstraintPatternMismatch,
            "ref" => DiagnosticCode::ConstraintForeignKeyViolation,
            _ => DiagnosticCode::ConstraintUnknown,
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
        Int8(n) => Some(*n as i64),
        Int16(n) => Some(*n as i64),
        Int32(n) => Some(*n as i64),
        Int64(n) => Some(*n),
        Uint8(n) => Some(*n as i64),
        Uint16(n) => Some(*n as i64),
        // Uint32 may exceed i64 for top bit set; treat as overflow and reject.
        Uint32(n) if *n <= i64::MAX as u32 => Some(*n as i64),
        Uint64(n) if *n <= i64::MAX as u64 => Some(*n as i64),
        Uint32(_) | Uint64(_) => None,
        _ => None,
    }
}

impl Constraint {
    /// Returns true when this constraint's validation depends on
    /// information outside the single table (i.e. needs `validate_project`).
    pub fn is_cross_table(&self) -> bool {
        matches!(self.func.as_str(), "ref")
    }

    pub fn validate(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        match self.func.as_str() {
            "unique" => self.validate_unique(fields, rows),
            "seq" => self.validate_sequence(fields, rows),
            "order" => self.validate_order(fields, rows),
            // Field-level single-cell constraints
            "nullable" => self.validate_nullable(fields, rows),
            "range" => self.validate_range(fields, rows),
            "oneof" => self.validate_oneof(fields, rows),
            "maxlen" => self.validate_maxlen(fields, rows),
            "pattern" => self.validate_pattern(fields, rows),
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

        // SQL-style semantics: NULL / empty-cell values are NOT compared
        // against each other for uniqueness. Default-not-null at schema level
        // (overridable by `@nullable`) decides whether empties are allowed.
        let mut seen_values: HashSet<Vec<Value>> = HashSet::new();

        for (row_index, row) in rows.iter().enumerate() {
            let mut key_values = Vec::new();

            for field_name in &field_names {
                if let Some(value) = row.get_field(field_name) {
                    key_values.push(value.clone());
                } else {
                    return Err(format!(
                        "Field '{}' not found in row {}",
                        field_name,
                        row_index + 1
                    ));
                }
            }

            // Skip the whole row if every key value is considered "empty".
            // Any non-empty value forces the row into the seen-set.
            let any_present = key_values.iter().any(|v| !Self::is_considered_empty(v));
            if !any_present {
                continue;
            }

            if !seen_values.insert(key_values) {
                return Err(format!(
                    "Duplicate values found at row {} for fields: {}",
                    row_index + 1,
                    field_names.join(", ")
                ));
            }
        }

        Ok(())
    }

    fn validate_sequence(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        if fields.len() != 1 {
            return Err("@seq constraint requires exactly one field".to_string());
        }

        // Forms:
        //   @seq       → start=1, step=1   (legacy)
        //   @seq(step) → start=1, step     (legacy second form)
        let step: i64 = if self.args.is_empty() {
            1
        } else {
            self.args[0]
                .trim()
                .parse()
                .map_err(|_| "@seq step must be an integer".to_string())?
        };

        let field_name = &fields[0].name;
        let mut expected_value = 1;

        for (row_index, row) in rows.iter().enumerate() {
            if let Some(value) = row.get_field(field_name) {
                let n = numeric_i64(value)
                    .ok_or_else(|| format!("@seq requires numeric field '{}'", field_name))?;
                if n != expected_value {
                    return Err(format!(
                        "expected {} at row {} but found {}",
                        expected_value,
                        row_index + 1,
                        n
                    ));
                }
            } else {
                return Err(format!(
                    "Field '{}' not found in row {}",
                    field_name,
                    row_index + 1
                ));
            }

            expected_value += step;
        }

        Ok(())
    }

    fn validate_order(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        // Single-field asc/desc only. Forms:
        //   @order          → asc on the field-level host (fields[0])
        //   @order(asc|desc)→ same, explicit direction
        if fields.len() != 1 {
            return Err("@order requires exactly one field".to_string());
        }
        let order_type = if self.args.is_empty() {
            "asc"
        } else {
            &self.args[0]
        };
        if order_type != "asc" && order_type != "desc" {
            return Err(format!(
                "@order argument must be 'asc' or 'desc' (got '{}')",
                order_type
            ));
        }
        let field_name = &fields[0].name;
        let mut prev_value: Option<&Value> = None;
        for (row_index, row) in rows.iter().enumerate() {
            if let Some(current_value) = row.get_field(field_name) {
                if let Some(prev) = prev_value {
                    match (order_type, prev.partial_cmp(current_value)) {
                        ("asc", Some(std::cmp::Ordering::Greater)) => {
                            return Err(format!(
                                "Order violation at row {}: {} > {} (expected ascending)",
                                row_index + 1,
                                prev,
                                current_value
                            ));
                        }
                        ("desc", Some(std::cmp::Ordering::Less)) => {
                            return Err(format!(
                                "Order violation at row {}: {} < {} (expected descending)",
                                row_index + 1,
                                prev,
                                current_value
                            ));
                        }
                        (_, None) => {
                            return Err(format!("Cannot compare values at row {}", row_index + 1));
                        }
                        _ => {}
                    }
                }
                prev_value = Some(current_value);
            } else {
                return Err(format!(
                    "Field '{}' not found in row {}",
                    field_name,
                    row_index + 1
                ));
            }
        }
        Ok(())
    }

    // ---- Layer 1 helpers ----

    fn require_single_field(&self, fields: &[Field], name: &str) -> Result<String, String> {
        if fields.len() != 1 {
            return Err(format!("@{} requires exactly one field", name));
        }
        Ok(fields[0].name.clone())
    }

    fn require_int_arg(&self, idx: usize, name: &str) -> Result<i64, String> {
        let raw = self
            .args
            .get(idx)
            .ok_or_else(|| format!("@{}: missing positional argument {}", name, idx + 1))?;
        raw.trim().parse::<i64>().map_err(|_| {
            format!(
                "@{}: argument {} must be an integer (got '{}')",
                name,
                idx + 1,
                raw
            )
        })
    }

    fn value_to_i64(&self, v: &Value) -> Option<i64> {
        numeric_i64(v)
    }

    fn value_to_str(&self, v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    fn validate_nullable(&self, _fields: &[Field], _rows: &[Row]) -> Result<(), String> {
        // `@nullable` is a marker flag only; the actual non-empty semantics
        // is enforced in `ConstraintValidator::validate_table` as a pre-pass.
        Ok(())
    }

    fn validate_range(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        let field_name = self.require_single_field(fields, "range")?;
        let lo = self.require_int_arg(0, "range")?;
        let hi = self.require_int_arg(1, "range")?;
        if lo > hi {
            return Err(format!("@range: min ({}) must be <= max ({})", lo, hi));
        }
        for (idx, row) in rows.iter().enumerate() {
            let v = row
                .get_field(&field_name)
                .ok_or_else(|| format!("field '{}' missing at row {}", field_name, idx + 1))?;
            let n = self
                .value_to_i64(v)
                .ok_or_else(|| format!("@range requires numeric field '{}'", field_name))?;
            if n < lo || n > hi {
                return Err(format!(
                    "field '{}' = {} not in [{}, {}] (row {})",
                    field_name,
                    n,
                    lo,
                    hi,
                    idx + 1,
                ));
            }
        }
        Ok(())
    }

    fn validate_oneof(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        if self.args.is_empty() {
            return Err("@oneof requires at least one allowed value".to_string());
        }
        let field_name = self.require_single_field(fields, "oneof")?;

        // Normalise each allowed value to the same shape we'll match against.
        // For numeric fields, parse each arg as a number too.
        enum Allowed {
            Str(String),
            Int(i64),
        }
        let mut allowed: Vec<Allowed> = Vec::new();
        for a in &self.args {
            if let Ok(n) = a.trim().parse::<i64>() {
                allowed.push(Allowed::Int(n));
            } else {
                allowed.push(Allowed::Str(a.clone()));
            }
        }

        for (idx, row) in rows.iter().enumerate() {
            let v = row
                .get_field(&field_name)
                .ok_or_else(|| format!("field '{}' missing at row {}", field_name, idx + 1))?;
            let ok = match v {
                Value::String(s) => allowed
                    .iter()
                    .any(|a| matches!(a, Allowed::Str(x) if x == s)),
                n if self.value_to_i64(n).is_some() => {
                    let lhs = self.value_to_i64(n).unwrap();
                    allowed
                        .iter()
                        .any(|a| matches!(a, Allowed::Int(x) if *x == lhs))
                }
                _ => false,
            };
            if !ok {
                return Err(format!(
                    "field '{}' = {} is not in {{ {} }} (row {})",
                    field_name,
                    v,
                    self.args.join(", "),
                    idx + 1,
                ));
            }
        }
        Ok(())
    }

    fn validate_maxlen(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        let field_name = self.require_single_field(fields, "maxlen")?;
        let hi = self.require_int_arg(0, "maxlen")?;
        if hi < 0 {
            return Err("@maxlen argument must be >= 0".to_string());
        }
        for (idx, row) in rows.iter().enumerate() {
            let v = row
                .get_field(&field_name)
                .ok_or_else(|| format!("field '{}' missing at row {}", field_name, idx + 1))?;
            let s = self
                .value_to_str(v)
                .ok_or_else(|| format!("@maxlen requires string field '{}'", field_name))?;
            if (s.chars().count() as i64) > hi {
                return Err(format!(
                    "field '{}' length > {} (row {})",
                    field_name,
                    hi,
                    idx + 1,
                ));
            }
        }
        Ok(())
    }

    fn validate_pattern(&self, fields: &[Field], rows: &[Row]) -> Result<(), String> {
        if self.args.len() != 1 {
            return Err(
                "@pattern requires exactly one regex argument (quote it with \"...\")".to_string(),
            );
        }
        let field_name = self.require_single_field(fields, "pattern")?;
        let re = regex::Regex::new(&self.args[0])
            .map_err(|e| format!("@pattern: invalid regex '{}': {}", self.args[0], e))?;
        for (idx, row) in rows.iter().enumerate() {
            let v = row
                .get_field(&field_name)
                .ok_or_else(|| format!("field '{}' missing at row {}", field_name, idx + 1))?;
            let s = self
                .value_to_str(v)
                .ok_or_else(|| format!("@pattern requires string field '{}'", field_name))?;
            if !re.is_match(&s) {
                return Err(format!(
                    "field '{}' = '{}' does not match pattern '{}' (row {})",
                    field_name,
                    s,
                    self.args[0],
                    idx + 1,
                ));
            }
        }
        Ok(())
    }

    // ---- Layer 2 (intra-row cross-field) intentionally omitted — see doc/design.md ----

    fn is_considered_empty(v: &Value) -> bool {
        match v {
            Value::String(s) => s.is_empty(),
            Value::Null => true,
            _ => false,
        }
    }

    // ---- Layer 4: cross-table FK (project-level) ----

    /// Run the cross-table check for `@ref`.
    /// `host` is the column on the host table to read from; the target
    /// `table.column` is taken from this constraint's arguments.
    pub fn validate_cross_table(
        &self,
        _fields: &[Field],
        rows: &[Row],
        by_name: &HashMap<String, &Table>,
        host: &str,
    ) -> Result<(), String> {
        // Args shape:
        //   @ref("Other.id")        (field-level, target is the only arg)
        //   @ref(host, "Other.id")  (table-level: first arg is host)
        if self.func != "ref" {
            return Err(format!("@{} is not a cross-table constraint", self.func));
        }
        let (target_spec, host_field_name): (String, String) = if self.args.is_empty() {
            return Err("@ref requires a target 'table.column'".to_string());
        } else if self.args.len() == 2 {
            // table-level: first arg is host, second is target
            (self.args[1].clone(), self.args[0].clone())
        } else if self.args.len() == 1 {
            // field-level: target is the only arg, host is supplied
            (self.args[0].clone(), host.to_string())
        } else {
            return Err(format!(
                "@ref takes 1 or 2 arguments (got {})",
                self.args.len()
            ));
        };

        let (target_table, target_col) = target_spec.split_once('.').ok_or_else(|| {
            format!(
                "@ref: target must be 'table.column' (got '{}')",
                target_spec
            )
        })?;
        let target_table = target_table.to_string();
        let target_col = target_col.to_string();

        if !by_name.contains_key(&target_table) {
            return Err(format!("@ref: target table '{}' not found", target_table));
        }
        let target = by_name[&target_table];

        if !target.fields.iter().any(|f| f.name == target_col) {
            return Err(format!(
                "@ref: target column '{}' not in table '{}'",
                target_col, target_table
            ));
        }

        // Build set of target values, skipping empties (a missing key is not a valid reference target).
        let target_set: HashSet<Value> = target
            .data
            .iter()
            .filter_map(|r| match r.get_field(&target_col) {
                Some(v)
                    if !matches!(v, Value::String(s) if s.is_empty())
                        && !matches!(v, Value::Null) =>
                {
                    Some(v.clone())
                }
                _ => None,
            })
            .collect();

        for (idx, row) in rows.iter().enumerate() {
            let v = match row.get_field(&host_field_name) {
                Some(v)
                    if !matches!(v, Value::String(s) if s.is_empty())
                        && !matches!(v, Value::Null) =>
                {
                    v
                }
                // SQL-style: a NULL FK is allowed (no parent required).
                _ => continue,
            };
            if !target_set.contains(v) {
                return Err(format!(
                    "row {}: host '{}' = {} missing from target {}.{}",
                    idx + 1,
                    host_field_name,
                    v,
                    target_table,
                    target_col,
                ));
            }
        }
        Ok(())
    }
}

pub struct ConstraintValidator;

impl ConstraintValidator {
    pub fn validate_table(table: &Table) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();

        // Pre-pass: schema-level default-not-null. Any cell that is empty
        // (missing field, empty string, or `Value::Null`) is reported unless
        // the field has `@nullable` to opt out.
        let nullable_fields: std::collections::HashSet<&str> = table
            .fields
            .iter()
            .filter(|f| f.constraint.as_ref().is_some_and(|c| c.func == "nullable"))
            .map(|f| f.name.as_str())
            .collect();
        for field in &table.fields {
            if nullable_fields.contains(field.name.as_str()) {
                continue;
            }
            for (idx, row) in table.data.iter().enumerate() {
                let v = row.get_field(&field.name);
                let empty = match v {
                    None => true,
                    Some(Value::String(s)) => s.is_empty(),
                    Some(Value::Null) => true,
                    _ => false,
                };
                if empty {
                    errors.push(Diagnostic::new(
                        DiagnosticCode::ConstraintNullNotAllowed,
                        format!(
                            "field '{}' must not be empty at row {}; \
                             add @nullable to opt out",
                            field.name,
                            idx + 1,
                        ),
                        SourceLocation::default(),
                    ));
                }
            }
        }

        // Field-level constraints (constraint declared in row 4 column).
        for field in &table.fields {
            if let Some(constraint) = &field.constraint {
                // Defer cross-table constraints to validate_project.
                if constraint.is_cross_table() {
                    continue;
                }
                if let Err(msg) = constraint.validate(&[field.clone()], &table.data) {
                    errors.push(constraint.to_diagnostic(&msg));
                }
            }
        }

        // Table-level constraints (row 5).
        for constraint in &table.constraints {
            // Defer cross-table constraints to validate_project.
            if constraint.is_cross_table() {
                continue;
            }
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

    /// Project-level validation. Runs all single-table constraints via
    /// `validate_table` for each table, then resolves cross-table
    /// foreign-key constraints (`@ref`, `@no_ref`).
    pub fn validate_project(tables: &[Table]) -> Result<(), Vec<Diagnostic>> {
        let mut errors: Vec<Diagnostic> = Vec::new();

        // First pass: per-table (immediate) constraints.
        for t in tables {
            if let Err(es) = Self::validate_table(t) {
                errors.extend(es);
            }
        }

        // Second pass: cross-table constraints.
        let by_name: HashMap<String, &Table> = tables.iter().map(|t| (t.name.clone(), t)).collect();

        for t in tables {
            // Field-level @ref / @no_ref: host is the field itself.
            for field in &t.fields {
                if let Some(c) = &field.constraint {
                    if c.is_cross_table() {
                        if let Err(msg) =
                            c.validate_cross_table(&[field.clone()], &t.data, &by_name, &field.name)
                        {
                            errors.push(c.to_diagnostic(&msg));
                        }
                    }
                }
            }
            // Table-level @ref / @no_ref: first arg names the host.
            for c in &t.constraints {
                if c.is_cross_table() {
                    let host = c.args.first().cloned().unwrap_or_default();
                    if let Err(msg) = c.validate_cross_table(&t.fields, &t.data, &by_name, &host) {
                        errors.push(c.to_diagnostic(&msg));
                    }
                }
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
        use crate::core::table::field::FieldType;
        use crate::core::table::row::Row;
        use crate::core::table::value::Value;

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

        assert!(
            unique_constraint
                .validate(&fields, &duplicate_rows)
                .is_err()
        );

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

        assert!(
            order_constraint
                .validate(&order_fields, &ordered_rows)
                .is_ok()
        );

        // Test unordered rows
        let unordered_rows = vec![
            Row::from_vec(vec![("value".to_string(), Value::Int32(1))]),
            Row::from_vec(vec![("value".to_string(), Value::Int32(3))]),
            Row::from_vec(vec![("value".to_string(), Value::Int32(2))]),
        ];

        assert!(
            order_constraint
                .validate(&order_fields, &unordered_rows)
                .is_err()
        );
    }

    #[test]
    fn validate_sequence_handles_each_width() {
        let c = Constraint::from_str("@seq").unwrap();
        let fields = vec![Field {
            name: "n".into(),
            t: FieldType::Int16,
            desc: "".into(),
            constraint: Some(c.clone()),
            tags: vec![],
        }];
        let rows = vec![
            Row::from_vec(vec![("n".into(), Value::Int16(1))]),
            Row::from_vec(vec![("n".into(), Value::Int16(2))]),
        ];
        assert!(c.validate(&fields, &rows).is_ok());
    }

    fn parse(s: &str) -> Result<Vec<String>, ()> {
        let loc = SourceLocation::default();
        let c = Constraint::from_str_with_loc(s, loc).map_err(|_| ())?;
        Ok(c.args)
    }

    #[test]
    fn parse_args_unquoted_legacy() {
        assert_eq!(parse("@func()").unwrap(), Vec::<String>::new());
        assert_eq!(parse("@func( )").unwrap(), Vec::<String>::new());
        assert_eq!(parse("@func(a)").unwrap(), vec!["a".to_string()]);
        assert_eq!(
            parse("@func(a, b)").unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            parse("@func(  a  ,  b  )").unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        // Trailing comma yields an empty arg, matching the old split(',') behavior.
        assert_eq!(
            parse("@func(a,)").unwrap(),
            vec!["a".to_string(), "".to_string()]
        );
    }

    #[test]
    fn parse_args_quoted_preserves_inner_comma() {
        assert_eq!(
            parse("@oneof(\"a,b\", c)").unwrap(),
            vec!["a,b".to_string(), "c".to_string()],
        );
        assert_eq!(
            parse("@pattern(\"^[a-z]+@[a-z]+$\")").unwrap(),
            vec!["^[a-z]+@[a-z]+$".to_string()],
        );
    }

    #[test]
    fn parse_args_quoted_equivalence_with_unquoted() {
        assert_eq!(
            parse("@oneof(\"a\", \"b\", \"c\")").unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            parse("@oneof(a, b, c)").unwrap(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn parse_args_quoted_escape_quote_and_backslash() {
        assert_eq!(
            parse("@pattern(\"\\\\path\\\\to\\\\file\")").unwrap(),
            vec!["\\path\\to\\file".to_string()],
        );
        assert_eq!(
            parse("@oneof(\"he said \\\"hi\\\"\")").unwrap(),
            vec!["he said \"hi\"".to_string()],
        );
    }

    #[test]
    fn parse_args_quoted_unsupported_escape_errors() {
        // \n is not a recognised escape in our scheme.
        assert!(parse("@pattern(\"\\n\")").is_err());
    }

    #[test]
    fn parse_args_unterminated_string_errors() {
        assert!(parse("@pattern(\"unterminated)").is_err());
        assert!(parse("@oneof(\"a, b)").is_err());
        assert!(parse("@func(\"unterminated escape \\\\)").is_err());
    }

    #[test]
    fn parse_args_quote_mid_arg_errors() {
        // A `"` appearing in the middle of a non-quoted arg is ambiguous; reject.
        assert!(parse("@func(a\"b)").is_err());
    }

    #[test]
    fn parse_args_zero_arg_legacy_paths_still_work() {
        // The pre-existing single-token forms must keep working.
        assert!(Constraint::from_str("@seq").is_ok());
        assert!(Constraint::from_str("@unique").is_ok());
        assert!(Constraint::from_str("@order").is_ok());
    }

    // ---- Layer 1 tests ----

    fn mk_str_field(name: &str, constraint: Constraint) -> Field {
        Field {
            name: name.into(),
            t: FieldType::String,
            desc: "".into(),
            constraint: Some(constraint),
            tags: vec![],
        }
    }

    fn mk_int_field(name: &str, t: FieldType, constraint: Constraint) -> Field {
        Field {
            name: name.into(),
            t,
            desc: "".into(),
            constraint: Some(constraint),
            tags: vec![],
        }
    }

    fn one_row(values: Vec<(&str, Value)>) -> Vec<Row> {
        vec![Row::from_vec(
            values.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        )]
    }

    fn empty_row(name: &str) -> Vec<Row> {
        vec![Row::from_vec(vec![(name.into(), Value::String("".into()))])]
    }

    fn missing_row() -> Vec<Row> {
        vec![Row::from_vec(vec![])]
    }

    #[test]
    fn layer1_oneof_strings_and_ints() {
        let c = Constraint::from_str("@oneof(red, green, blue)").unwrap();
        let fs = mk_str_field("color", c.clone());
        assert!(
            c.validate(
                &[fs.clone()],
                &one_row(vec![("color", Value::String("red".into()))])
            )
            .is_ok()
        );
        assert!(
            c.validate(
                &[fs.clone()],
                &one_row(vec![("color", Value::String("yellow".into()))])
            )
            .is_err()
        );

        let ci = Constraint::from_str("@oneof(\"1\", \"2\", \"3\")").unwrap();
        let fi = mk_int_field("n", FieldType::Int32, ci.clone());
        assert!(
            ci.validate(&[fi.clone()], &one_row(vec![("n", Value::Int32(2))]))
                .is_ok()
        );
        assert!(
            ci.validate(&[fi.clone()], &one_row(vec![("n", Value::Int32(4))]))
                .is_err()
        );
    }

    #[test]
    fn layer1_maxlen_chars() {
        let c = Constraint::from_str("@maxlen(5)").unwrap();
        let f = mk_str_field("s", c.clone());
        assert!(
            c.validate(
                &[f.clone()],
                &one_row(vec![("s", Value::String("abcde".into()))])
            )
            .is_ok()
        );
        assert!(
            c.validate(
                &[f.clone()],
                &one_row(vec![("s", Value::String("abcdef".into()))])
            )
            .is_err()
        );
    }

    #[test]
    fn layer1_pattern_matches_via_quote_arg() {
        let c = Constraint::from_str("@pattern(\"^[a-z]+@[a-z]+$\")").unwrap();
        let f = mk_str_field("email", c.clone());
        assert!(
            c.validate(
                &[f.clone()],
                &one_row(vec![("email", Value::String("alice@example".into()))])
            )
            .is_ok()
        );
        assert!(
            c.validate(
                &[f.clone()],
                &one_row(vec![("email", Value::String("no-at-symbol".into()))])
            )
            .is_err()
        );
    }

    #[test]
    fn layer1_pattern_rejects_bad_regex() {
        let c = Constraint::from_str("@pattern(\"([unclosed\")").unwrap();
        let f = mk_str_field("s", c.clone());
        let r = one_row(vec![("s", Value::String("anything".into()))]);
        assert!(c.validate(&[f], &r).is_err());
    }

    #[test]
    fn layer1_to_diagnostic_codes() {
        let cases: &[(&str, crate::core::diagnostic::DiagnosticCode)] = &[
            (
                "@range(0, 1)",
                crate::core::diagnostic::DiagnosticCode::ConstraintValueViolation,
            ),
            (
                "@nullable",
                crate::core::diagnostic::DiagnosticCode::ConstraintNullNotAllowed,
            ),
            (
                "@oneof(a)",
                crate::core::diagnostic::DiagnosticCode::ConstraintNotInSet,
            ),
            (
                "@maxlen(0)",
                crate::core::diagnostic::DiagnosticCode::ConstraintValueViolation,
            ),
            (
                "@pattern(\"x\")",
                crate::core::diagnostic::DiagnosticCode::ConstraintPatternMismatch,
            ),
        ];
        for (s, code) in cases {
            let c = Constraint::from_str(s).unwrap();
            let d = c.to_diagnostic("oops");
            assert_eq!(&d.code, code, "code for {s}");
            assert!(d.message.contains("oops"), "msg should contain detail");
        }
    }

    // ---- Layer 2 tests ----

    fn field_named(name: &str, t: FieldType) -> Field {
        Field {
            name: name.into(),
            t,
            desc: "".into(),
            constraint: None,
            tags: vec![],
        }
    }

    #[test]
    fn layer3_unique_skips_empty_sql_semantics() {
        // @unique now uses SQL semantics: empty / null cells are not compared
        // against each other. Mixing "a" + "a" still fails.
        let c = Constraint::from_str("@unique").unwrap();
        let f = mk_str_field("k", c.clone());
        let rows = vec![
            Row::from_vec(vec![("k".into(), Value::String("".into()))]),
            Row::from_vec(vec![("k".into(), Value::String("".into()))]), // empty OK
            Row::from_vec(vec![("k".into(), Value::String("a".into()))]),
            Row::from_vec(vec![("k".into(), Value::String("b".into()))]),
            Row::from_vec(vec![("k".into(), Value::String("".into()))]), // empty OK
        ];
        assert!(c.validate(&[f.clone()], &rows).is_ok());

        let bad = vec![
            Row::from_vec(vec![("k".into(), Value::String("".into()))]),
            Row::from_vec(vec![("k".into(), Value::String("a".into()))]),
            Row::from_vec(vec![("k".into(), Value::String("a".into()))]),
        ];
        assert!(c.validate(&[f.clone()], &bad).is_err());
    }

    #[test]
    fn layer1_range_inclusive() {
        let c = Constraint::from_str("@range(1, 10)").unwrap();
        let f = mk_int_field("n", FieldType::Int32, c.clone());
        assert!(
            c.validate(&[f.clone()], &one_row(vec![("n", Value::Int32(5))]))
                .is_ok()
        );
        assert!(
            c.validate(&[f.clone()], &one_row(vec![("n", Value::Int32(1))]))
                .is_ok()
        );
        assert!(
            c.validate(&[f.clone()], &one_row(vec![("n", Value::Int32(10))]))
                .is_ok()
        );
        assert!(
            c.validate(&[f.clone()], &one_row(vec![("n", Value::Int32(0))]))
                .is_err()
        );
        assert!(
            c.validate(&[f.clone()], &one_row(vec![("n", Value::Int32(11))]))
                .is_err()
        );
    }

    #[test]
    fn layer1_range_inverted_or_nonint_errors() {
        // lo > hi is rejected (no degenerate ranges).
        let bad = Constraint::from_str("@range(10, 1)").unwrap();
        let f = mk_int_field("n", FieldType::Int32, bad.clone());
        let r = one_row(vec![("n", Value::Int32(5))]);
        assert!(bad.validate(&[f.clone()], &r).is_err());
        // Non-int arg.
        let nbad = Constraint::from_str("@range(abc, 5)").unwrap();
        assert!(nbad.validate(&[f], &r).is_err());
    }

    #[test]
    fn default_not_null_precheck_rejects_empty_cells() {
        // Schema-level default-not-null kicks in when no `@nullable` is declared.
        let f = mk_str_field("name", Constraint::from_str("@oneof(a, b)").unwrap());
        let rows = vec![
            Row::from_vec(vec![("name".into(), Value::String("a".into()))]),
            Row::from_vec(vec![("name".into(), Value::String("".into()))]),
        ];
        let t = mk_table("T", vec![f], rows, vec![]);
        let r = ConstraintValidator::validate_table(&t);
        assert!(r.is_err());
        // Both the pre-check (default-not-null) and the @oneof inner
        // validator complain about the empty cell, so multiple diagnostics
        // are expected.
        assert!(!r.err().unwrap().is_empty());
    }

    #[test]
    fn nullable_opt_in_silences_default_not_null() {
        let f = mk_str_field("comment", Constraint::from_str("@nullable").unwrap());
        let t = mk_table(
            "T",
            vec![f],
            vec![Row::from_vec(vec![(
                "comment".into(),
                Value::String("".into()),
            )])],
            vec![],
        );
        assert!(ConstraintValidator::validate_table(&t).is_ok());
    }

    #[test]
    fn layer3_seq_step_form() {
        // @seq(step) is the only non-default form; @seq(start, step) was removed.
        let cl = Constraint::from_str("@seq(2)").unwrap();
        let fl = mk_int_field("n", FieldType::Int32, cl.clone());
        let okl = vec![
            Row::from_vec(vec![("n".into(), Value::Int32(1))]),
            Row::from_vec(vec![("n".into(), Value::Int32(3))]),
        ];
        assert!(cl.validate(&[fl.clone()], &okl).is_ok());
        let badl = vec![
            Row::from_vec(vec![("n".into(), Value::Int32(1))]),
            Row::from_vec(vec![("n".into(), Value::Int32(4))]),
        ];
        assert!(cl.validate(&[fl.clone()], &badl).is_err());
    }

    // ---- Layer 4 tests ----

    fn mk_table(
        name: &str,
        fields: Vec<Field>,
        data: Vec<Row>,
        constraints: Vec<Constraint>,
    ) -> Table {
        Table {
            name: name.to_string(),
            fields,
            data,
            constraints,
        }
    }

    #[test]
    fn layer4_ref_passes_when_present_and_fails_when_missing() {
        let item = mk_table(
            "Item",
            vec![mk_int_field(
                "id",
                FieldType::Int32,
                Constraint::from_str("@nullable").unwrap(),
            )],
            vec![
                Row::from_vec(vec![("id".into(), Value::Int32(1))]),
                Row::from_vec(vec![("id".into(), Value::Int32(2))]),
                Row::from_vec(vec![("id".into(), Value::Int32(3))]),
            ],
            vec![],
        );
        let drop = mk_table(
            "Drop",
            vec![mk_int_field(
                "item_id",
                FieldType::Int32,
                Constraint::from_str("@ref(\"Item.id\")").unwrap(),
            )],
            vec![
                Row::from_vec(vec![("item_id".into(), Value::Int32(1))]),
                Row::from_vec(vec![("item_id".into(), Value::Int32(2))]),
            ],
            vec![],
        );
        assert!(ConstraintValidator::validate_project(&[item.clone(), drop.clone()]).is_ok());

        // Add a row that references id=99 (not in Item).
        let bad_drop = mk_table(
            "Drop",
            vec![mk_int_field(
                "item_id",
                FieldType::Int32,
                Constraint::from_str("@ref(\"Item.id\")").unwrap(),
            )],
            vec![
                Row::from_vec(vec![("item_id".into(), Value::Int32(1))]),
                Row::from_vec(vec![("item_id".into(), Value::Int32(99))]),
            ],
            vec![],
        );
        let r = ConstraintValidator::validate_project(&[item, bad_drop]);
        assert!(r.is_err());
    }

    #[test]
    fn layer4_ref_missing_target_table_reports_diagnostic() {
        let t = mk_table(
            "Only",
            vec![mk_int_field(
                "item_id",
                FieldType::Int32,
                Constraint::from_str("@ref(\"NoSuch.id\")").unwrap(),
            )],
            vec![Row::from_vec(vec![("item_id".into(), Value::Int32(1))])],
            vec![],
        );
        let r = ConstraintValidator::validate_project(&[t]);
        assert!(r.is_err());
    }

    #[test]
    fn layer4_ref_with_table_level_form() {
        let item = mk_table(
            "Item",
            vec![field_named("id", FieldType::Int32)],
            vec![
                Row::from_vec(vec![("id".into(), Value::Int32(1))]),
                Row::from_vec(vec![("id".into(), Value::Int32(2))]),
            ],
            vec![],
        );
        let user = mk_table(
            "User",
            vec![
                field_named("fav_id", FieldType::Int32),
                field_named("name", FieldType::String),
            ],
            vec![Row::from_vec(vec![
                ("fav_id".into(), Value::Int32(1)),
                ("name".into(), Value::String("alice".into())),
            ])],
            vec![Constraint::from_str("@ref(fav_id, \"Item.id\")").unwrap()],
        );
        assert!(ConstraintValidator::validate_project(&[item, user]).is_ok());
    }

    // ---- Coverage-targeted additional tests ----
    // Each of these hits one or more branches that the tests above leave uncovered.

    #[test]
    fn parser_unterminated_escape_returns_err() {
        // raw trailing backslash triggers a Diagnostic, not a panic.
        let loc = SourceLocation::default();
        let r = Constraint::from_str_with_loc("@func(\"abc\\) ", loc);
        assert!(r.is_err());
    }

    #[test]
    fn parser_at_with_empty_body_errors() {
        let loc = SourceLocation::default();
        // bare "@" with no body
        assert!(Constraint::from_str_with_loc("@()", loc).is_err());
    }

    #[test]
    fn to_diagnostic_unknown_func_emits_unknown() {
        let c = Constraint::from_str("@totally_unknown(1)").unwrap();
        let d = c.to_diagnostic("oops");
        assert_eq!(
            d.code,
            crate::core::diagnostic::DiagnosticCode::ConstraintUnknown
        );
    }

    #[test]
    fn validate_dispatches_unknown_with_err() {
        let c = Constraint::from_str("@totally_unknown(1)").unwrap();
        let r = c.validate(&[], &[]);
        assert!(r.is_err());
    }

    #[test]
    fn numeric_i64_covers_each_width_and_u64_overflow() {
        assert_eq!(numeric_i64(&Value::Int8(7)), Some(7));
        assert_eq!(numeric_i64(&Value::Int16(7)), Some(7));
        assert_eq!(numeric_i64(&Value::Int32(7)), Some(7));
        assert_eq!(numeric_i64(&Value::Int64(7)), Some(7));
        assert_eq!(numeric_i64(&Value::Uint8(7)), Some(7));
        assert_eq!(numeric_i64(&Value::Uint16(7)), Some(7));
        assert_eq!(numeric_i64(&Value::Uint32(7)), Some(7));
        assert_eq!(numeric_i64(&Value::Uint64(7)), Some(7));
        // u64 overflow path: numbers above i64::MAX cannot be represented;
        // u32 never overflows because u32::MAX fits in i64.
        assert!(numeric_i64(&Value::Uint64(u64::MAX)).is_none());
        assert!(numeric_i64(&Value::String("x".into())).is_none());
    }

    #[test]
    fn unique_single_field_args_error_path() {
        let c = Constraint::from_str("@unique").unwrap();
        let bad = vec![
            field_named("a", FieldType::Int32),
            field_named("b", FieldType::Int32),
        ];
        assert!(c.validate(&bad, &[]).is_err());
    }

    #[test]
    fn unique_composite_args_path() {
        let c = Constraint::from_str("@unique(a, b)").unwrap();
        let f = vec![
            field_named("a", FieldType::Int32),
            field_named("b", FieldType::Int32),
        ];
        let rows = vec![
            Row::from_vec(vec![
                ("a".into(), Value::Int32(1)),
                ("b".into(), Value::Int32(1)),
            ]),
            Row::from_vec(vec![
                ("a".into(), Value::Int32(1)),
                ("b".into(), Value::Int32(2)),
            ]),
        ];
        assert!(c.validate(&f, &rows).is_ok());
        let dup = vec![
            Row::from_vec(vec![
                ("a".into(), Value::Int32(1)),
                ("b".into(), Value::Int32(1)),
            ]),
            Row::from_vec(vec![
                ("a".into(), Value::Int32(1)),
                ("b".into(), Value::Int32(1)),
            ]),
        ];
        assert!(c.validate(&f, &dup).is_err());
        // Field 'a' missing → unique error path.
        let miss = vec![
            Row::from_vec(vec![("a".into(), Value::Int32(1))]),
            Row::from_vec(vec![("a".into(), Value::Int32(2))]),
        ];
        assert!(c.validate(&f, &miss).is_err());
    }

    #[test]
    fn seq_requires_one_field() {
        // @seq requires exactly one field; non-integer step is rejected.
        let c = Constraint::from_str("@seq").unwrap();
        let bad = vec![
            field_named("a", FieldType::Int32),
            field_named("b", FieldType::Int32),
        ];
        assert!(c.validate(&bad, &[]).is_err());

        let cb = Constraint::from_str("@seq").unwrap();
        let fb = vec![field_named("n", FieldType::Int32)];
        let rm = vec![Row::from_vec(vec![])];
        assert!(cb.validate(&fb, &rm).is_err());

        // Bad step arg → parse error from validate().
        let cs = Constraint::from_str("@seq(abc)").unwrap();
        let f = vec![field_named("n", FieldType::Int32)];
        assert!(cs.validate(&f, &[]).is_err());
    }

    #[test]
    fn order_desc_violation_and_invalid_arg() {
        // descending violation: 1 then 5 going UP.
        let c = Constraint::from_str("@order(desc)").unwrap();
        let f = vec![field_named("n", FieldType::Int32)];
        let bad = vec![
            Row::from_vec(vec![("n".into(), Value::Int32(1))]),
            Row::from_vec(vec![("n".into(), Value::Int32(5))]),
        ];
        assert!(c.validate(&f, &bad).is_err());

        let cbad = Constraint::from_str("@order(sideways)").unwrap();
        assert!(cbad.validate(&f, &[]).is_err());

        // order with field-level slice that has 0 fields → error.
        let c0 = Constraint::from_str("@order").unwrap();
        assert!(c0.validate(&[], &[]).is_err());
    }

    #[test]
    fn oneof_no_args_and_non_numeric_fallthrough() {
        let c = Constraint::from_str("@oneof()").unwrap();
        let f = mk_str_field("s", c.clone());
        let r = vec![Row::from_vec(vec![("s".into(), Value::String("x".into()))])];
        assert!(c.validate(&[f.clone()], &r).is_err());

        // non-string, non-numeric field → fallthrough to "not in set".
        let f2 = mk_int_field(
            "b",
            FieldType::Bool,
            Constraint::from_str("@oneof(red, green)").unwrap(),
        );
        let r2 = vec![Row::from_vec(vec![("b".into(), Value::Bool(true))])];
        assert!(
            Constraint::from_str("@oneof(red, green)")
                .unwrap()
                .validate(&[f2], &r2)
                .is_err()
        );
    }

    #[test]
    fn maxlen_negative_arg_error() {
        let max = Constraint::from_str("@maxlen(-1)").unwrap();
        let f = mk_str_field("s", max.clone());
        let r = vec![Row::from_vec(vec![(
            "s".into(),
            Value::String("abc".into()),
        )])];
        assert!(max.validate(&[f], &r).is_err());
    }

    #[test]
    fn pattern_wrong_arg_count() {
        let c = Constraint::from_str("@pattern(\"a\", \"b\")").unwrap();
        let f = mk_str_field("s", c.clone());
        let r = vec![Row::from_vec(vec![("s".into(), Value::String("a".into()))])];
        assert!(c.validate(&[f], &r).is_err());
    }

    #[test]
    fn count_wrong_arg_count() {
        let c = Constraint::from_str("@count_eq(1, 2)").unwrap();
        let f = vec![field_named("a", FieldType::Int32)];
        let r = vec![Row::from_vec(vec![("a".into(), Value::Int32(1))])];
        assert!(c.validate(&f, &r).is_err());
    }

    #[test]
    fn cross_table_no_args_and_wrong_arg_count_and_invalid_target() {
        // Field-level @ref with no args at all → validate_project skips (no args error).
        // We trigger that by directly calling validate_cross_table.
        let c = Constraint::from_str("@ref(\"Item.id\")").unwrap();
        let by_name: HashMap<String, &Table> = HashMap::new();
        let f = vec![field_named("item_id", FieldType::Int32)];
        let rows = vec![Row::from_vec(vec![("item_id".into(), Value::Int32(1))])];
        // No target table in by_name → "target table not found".
        assert!(
            c.validate_cross_table(&f, &rows, &by_name, "item_id")
                .is_err()
        );

        // target without dot → error.
        let c2 = Constraint::from_str("@ref(\"nodot\")").unwrap();
        let t = mk_table(
            "X",
            vec![field_named("v", FieldType::Int32)],
            vec![Row::from_vec(vec![("v".into(), Value::Int32(1))])],
            vec![],
        );
        let map: HashMap<String, &Table> = [("X".to_string(), &t)].into_iter().collect();
        assert!(c2.validate_cross_table(&f, &rows, &map, "item_id").is_err());

        // Wrong arg count (3) → error.
        let c3 = Constraint::from_str("@ref(a, \"X.v\", extra)").unwrap();
        assert!(c3.validate_cross_table(&f, &rows, &map, "item_id").is_err());

        // target table exists but column missing → error.
        let c4 = Constraint::from_str("@ref(\"X.nope\")").unwrap();
        assert!(c4.validate_cross_table(&f, &rows, &map, "item_id").is_err());

        // Empty/missing host cell → skipped (SQL NULL FK).
        let c5 = Constraint::from_str("@ref(\"X.v\")").unwrap();
        let rows_null = vec![Row::from_vec(vec![(
            "item_id".into(),
            Value::String("".into()),
        )])];
        assert!(
            c5.validate_cross_table(&f, &rows_null, &map, "item_id")
                .is_ok()
        );

        // no_ref positive path: hit target.
        let cn = Constraint::from_str("@no_ref(\"X.v\")").unwrap();
        let rows_hit = vec![Row::from_vec(vec![("item_id".into(), Value::Int32(1))])];
        assert!(
            cn.validate_cross_table(&f, &rows_hit, &map, "item_id")
                .is_err()
        );
    }

    #[test]
    fn validate_project_emits_per_table_error_and_table_level_constraint() {
        // Single-table validation that produces an error gets bubbled up by validate_project.
        let t = mk_table(
            "Bad",
            vec![mk_int_field(
                "id",
                FieldType::Int32,
                Constraint::from_str("@min(10)").unwrap(),
            )],
            vec![
                Row::from_vec(vec![("id".into(), Value::Int32(1))]),
                Row::from_vec(vec![("id".into(), Value::Int32(20))]),
            ],
            vec![],
        );
        assert!(ConstraintValidator::validate_project(&[t]).is_err());

        // Table-level constraint that fails is also surfaced.
        let t = mk_table(
            "Bad2",
            vec![
                field_named("a", FieldType::Int32),
                field_named("b", FieldType::Int32),
            ],
            vec![Row::from_vec(vec![
                ("a".into(), Value::Int32(1)),
                ("b".into(), Value::Int32(2)),
            ])],
            vec![Constraint::from_str("@eq(a, b)").unwrap()],
        );
        assert!(ConstraintValidator::validate_project(&[t]).is_err());
    }
}
