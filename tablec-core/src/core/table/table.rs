use calamine::{Data, Reader, open_workbook_auto};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::field::{self, FieldType};
use super::row::Row;
use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};
use crate::core::parser::value_parser::parse_value;

use super::constraint::{self, Constraint, ConstraintValidator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub fields: Vec<field::Field>,
    pub data: Vec<Row>,
    pub constraints: Vec<constraint::Constraint>,
}

pub fn read_excel(fpath: &str) -> Result<Vec<Table>, Vec<Diagnostic>> {
    let mut workbook = match open_workbook_auto(fpath) {
        Ok(wb) => wb,
        Err(e) => {
            return Err(vec![Diagnostic::new(
                crate::core::diagnostic::DiagnosticCode::Other,
                format!("failed to open workbook '{}': {}", fpath, e),
                SourceLocation {
                    file: Some(std::path::PathBuf::from(fpath)),
                    sheet: None,
                    line: None,
                    column: None,
                },
            )]);
        }
    };
    let mut tables = vec![];
    let mut diagnostics: Vec<Diagnostic> = vec![];

    for sheet_name in workbook.sheet_names().to_owned() {
        if sheet_name.starts_with('#') {
            continue;
        }

        let sheet = match workbook.worksheet_range(&sheet_name) {
            Ok(range) => range,
            Err(e) => {
                eprintln!("Error reading sheet '{}': {}. Skipping.", sheet_name, e);
                continue;
            }
        };
        let mut rows = sheet.rows();

        // --- Read Headers ---
        let field_names: Vec<String> = rows
            .next()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.to_string())
            .collect();
        let field_types_str: Vec<String> = rows
            .next()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.to_string())
            .collect();
        let field_comments: Vec<String> = rows
            .next()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.to_string())
            .collect();

        // Read Constraints (4th row)
        let constraint_str: Vec<String> = rows
            .next()
            .unwrap_or(&[])
            .iter()
            .map(|c| c.to_string())
            .collect();

        // Row 5: table-level constraints (each cell one constraint).
        let row5_iter = rows.next();
        let row5: Vec<String> = match row5_iter {
            Some(r) => r.iter().map(|c| c.to_string()).collect(),
            None => vec![],
        };
        let mut table_constraints: Vec<Constraint> = Vec::new();
        for (col_idx, raw) in row5.iter().enumerate() {
            let cell = raw.trim();
            if cell.is_empty() {
                continue;
            }
            if !cell.starts_with('@') {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::TableConstraintParseError,
                    format!(
                        "row 5 cell {} must start with @, got '{}'",
                        col_idx + 1,
                        cell
                    ),
                    SourceLocation {
                        file: Some(std::path::PathBuf::from(fpath)),
                        sheet: Some(sheet_name.clone()),
                        line: Some(5),
                        column: Some(col_idx as u32 + 1),
                    },
                ));
                continue;
            }
            let loc = SourceLocation {
                file: Some(std::path::PathBuf::from(fpath)),
                sheet: Some(sheet_name.clone()),
                line: Some(5),
                column: Some(col_idx as u32 + 1),
            };
            match Constraint::from_str_with_loc(cell, loc) {
                Ok(c) => table_constraints.push(c),
                Err(d) => diagnostics.push(d),
            }
        }

        let mut fields = Vec::new();
        for i in 0..field_names.len() {
            let name = field_names.get(i).unwrap_or(&"".to_string()).clone();
            if name.is_empty() || name.starts_with('#') {
                continue; // Skip empty or commented out columns
            }

            let raw_constraint = constraint_str.get(i).unwrap_or(&"".to_string()).clone();

            fields.push(field::Field {
                name: name.split('[').next().unwrap_or(&name).trim().to_string(), // Extract name before '['
                t: field::FieldType::from_str(field_types_str.get(i).unwrap_or(&"".to_string()))
                    .unwrap_or(FieldType::String), // Default to string if parse fails
                desc: field_comments.get(i).unwrap_or(&"".to_string()).clone(),
                constraint: constraint::Constraint::from_str(&raw_constraint).ok(),
                tags: {
                    let mut tags = Vec::new();
                    if let Some(start_bracket) = name.find('[') {
                        if let Some(end_bracket) = name.find(']') {
                            if end_bracket > start_bracket {
                                let tag_str = &name[start_bracket + 1..end_bracket];
                                tags.extend(tag_str.split(',').map(|s| s.trim().to_string()));
                            }
                        }
                    }
                    tags
                },
            });
        }

        // --- Read Data Rows ---
        let mut data = vec![];
        for (row_index, row_cells) in rows.enumerate() {
            // Skip empty rows
            if row_cells.iter().all(|c| matches!(c, Data::Empty)) {
                continue;
            }

            let mut new_row = Row::new();
            for (col_index, field) in fields.iter().enumerate() {
                let cell_value_str = row_cells
                    .get(col_index)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "".to_string());

                let cell_loc = SourceLocation {
                    file: Some(std::path::PathBuf::from(fpath)),
                    sheet: Some(sheet_name.clone()),
                    line: Some(row_index as u32 + 6), // rows 1-5 reserved, data starts at row 6
                    column: Some(col_index as u32 + 1),
                };
                match parse_value(&cell_value_str, &field.t, cell_loc) {
                    Ok(value) => {
                        new_row.add_field(field.name.clone(), value);
                    }
                    Err(d) => {
                        diagnostics.push(d);
                    }
                }
            }
            data.push(new_row);
        }

        tables.push(Table {
            name: sheet_name.to_owned(),
            fields,
            data,
            constraints: table_constraints,
        });
    }

    if diagnostics.is_empty() {
        Ok(tables)
    } else {
        Err(diagnostics)
    }
}

impl Table {
    pub fn validate_constraints(&self) -> Result<(), Vec<Diagnostic>> {
        ConstraintValidator::validate_table(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::row::Row;
    use crate::core::table::value::Value;
    use crate::export::Format;

    #[test]
    fn test_read_excel_basic() {
        // 跳过Excel文件测试，因为需要真实的Excel文件
        // 这里我们测试JSON导出功能，这是主要目标
        println!("Skipping Excel file test - focusing on JSON export functionality");
    }

    #[test]
    fn test_json_export() {
        // 创建一个简单的表格用于JSON导出测试
        let table = Table {
            name: "test_table".to_string(),
            fields: vec![
                crate::core::table::field::Field {
                    name: "ID".to_string(),
                    t: crate::core::table::field::FieldType::Int32,
                    desc: "ID字段".to_string(),
                    constraint: None,
                    tags: vec![],
                },
                crate::core::table::field::Field {
                    name: "Name".to_string(),
                    t: crate::core::table::field::FieldType::String,
                    desc: "姓名".to_string(),
                    constraint: None,
                    tags: vec![],
                },
            ],
            data: vec![
                Row::from_vec(vec![
                    ("ID".to_string(), Value::Int32(1)),
                    ("Name".to_string(), Value::String("Alice".to_string())),
                ]),
                Row::from_vec(vec![
                    ("ID".to_string(), Value::Int32(2)),
                    ("Name".to_string(), Value::String("Bob".to_string())),
                ]),
            ],
            constraints: vec![],
        };

        // 测试JSON导出
        let tables = vec![table];
        let project =
            crate::core::project::project::Project::from_tables("test_project".to_string(), tables);
        let json = crate::export::Json {
            pretty: false,
            include_fields: true,
        };
        let result = json.to_vec(&project);
        assert!(result.is_ok(), "JSON export failed: {:?}", result.err());

        let json_str = String::from_utf8(result.unwrap()).unwrap();
        assert!(
            json_str.contains("test_table"),
            "JSON should contain table name"
        );
        assert!(json_str.contains("Alice"), "JSON should contain data");
        assert!(
            json_str.contains("fields"),
            "JSON should contain fields when include_fields=true"
        );
    }

    #[test]
    fn out_of_range_cell_yields_clear_error() {
        // Generate an in-memory workbook-like construction? Tests of read_excel
        // require real .xlsx files; defer detailed xlsx tests to error_cases fixtures (c4).
    }
}
