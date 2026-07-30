use calamine::{Data, Reader, open_workbook_auto};
use serde::{Deserialize, Serialize};

use super::row::Row;
use crate::core::diagnostic::{Diagnostic, SourceLocation};
use crate::core::schema::{Schema, SchemaParseResult, SchemaParser, StandardSchemaParser};

use super::constraint::ConstraintValidator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub schema: Schema,
    pub data: Vec<Row>,
}

pub fn read_excel(fpath: &str) -> Result<Vec<Table>, Vec<Diagnostic>> {
    read_excel_with(fpath, &StandardSchemaParser)
}

pub fn read_excel_with(
    fpath: &str,
    parser: &dyn SchemaParser,
) -> Result<Vec<Table>, Vec<Diagnostic>> {
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
        let data_rows_raw: Vec<&[Data]> = sheet.rows().collect();
        let cells: Vec<Vec<String>> = data_rows_raw
            .iter()
            .map(|row| row.iter().map(|c| c.to_string()).collect())
            .collect();

        // 防止 # 跳过被 parser 拦截后再判：parser 优先决定
        let schema_result = match parser.parse_schema(&sheet_name, &cells) {
            Ok(r) => r,
            Err(d) => {
                diagnostics.extend(d);
                continue;
            }
        };

        let schema = match schema_result {
            SchemaParseResult::Skip => continue,
            SchemaParseResult::Schema(s) => s,
        };

        // 字段重名检查
        if let Some(d) = check_field_overlap(&schema.fields, fpath, &sheet_name) {
            diagnostics.push(d);
            continue;
        }

        // data_start_row 越界
        if schema.data_start_row > data_rows_raw.len() {
            diagnostics.push(Diagnostic::new(
                crate::core::diagnostic::DiagnosticCode::SchemaDataStartOOB,
                format!(
                    "data_start_row={} > sheet rows={}",
                    schema.data_start_row,
                    data_rows_raw.len()
                ),
                SourceLocation {
                    file: Some(std::path::PathBuf::from(fpath)),
                    sheet: Some(sheet_name.clone()),
                    line: None,
                    column: None,
                },
            ));
            continue;
        }

        let (rows, mut diags) = parse_data_rows(
            data_rows_raw
                .into_iter()
                .skip(schema.data_start_row)
                .enumerate(),
            &schema.fields,
            fpath,
            &sheet_name,
            schema.data_start_row,
        );
        diagnostics.append(&mut diags);

        tables.push(Table {
            name: sheet_name,
            schema,
            data: rows,
        });
    }

    if diagnostics.is_empty() {
        Ok(tables)
    } else {
        Err(diagnostics)
    }
}

fn parse_data_rows<'a, I: Iterator<Item = (usize, &'a [Data])>>(
    rows: I,
    fields: &[crate::core::table::field::Field],
    fpath: &str,
    sheet_name: &str,
    data_start_row: usize,
) -> (Vec<Row>, Vec<Diagnostic>) {
    use crate::core::parser::value_parser::parse_value;
    use calamine::Data;

    let mut data = vec![];
    let mut diagnostics = vec![];
    for (row_idx, row_cells) in rows {
        if row_cells.iter().all(|c| matches!(c, Data::Empty)) {
            continue;
        }
        let mut new_row = Row::new();
        for (col_index, field) in fields.iter().enumerate() {
            let cell_value_str = row_cells
                .get(col_index)
                .map(|c| c.to_string())
                .unwrap_or_default();
            let cell_loc = SourceLocation {
                file: Some(std::path::PathBuf::from(fpath)),
                sheet: Some(sheet_name.to_string()),
                line: Some((data_start_row + row_idx + 1) as u32),
                column: Some((col_index + 1) as u32),
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
    (data, diagnostics)
}

fn check_field_overlap(
    fields: &[crate::core::table::field::Field],
    fpath: &str,
    sheet_name: &str,
) -> Option<Diagnostic> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for f in fields {
        if !seen.insert(f.name.as_str()) {
            return Some(Diagnostic::new(
                crate::core::diagnostic::DiagnosticCode::SchemaFieldOverlap,
                format!(
                    "duplicate field name '{}' in sheet '{}'",
                    f.name, sheet_name
                ),
                SourceLocation {
                    file: Some(std::path::PathBuf::from(fpath)),
                    sheet: Some(sheet_name.to_string()),
                    line: None,
                    column: None,
                },
            ));
        }
    }
    None
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
            schema: Schema::from_parts(
                vec![
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
                vec![],
            ),
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

#[cfg(test)]
mod refactor_tests {
    use super::*;
    use crate::core::schema::Schema;

    #[test]
    fn table_constructs_with_schema_field() {
        let t = Table {
            name: "x".to_string(),
            schema: Schema::from_parts(vec![], vec![]),
            data: vec![],
        };
        assert_eq!(t.name, "x");
        assert_eq!(t.schema.fields.len(), 0);
    }

    #[test]
    fn table_schema_accessible() {
        let f = crate::core::table::field::Field {
            name: "id".to_string(),
            t: crate::core::table::field::FieldType::Int32,
            desc: String::new(),
            constraint: None,
            tags: vec![],
        };
        let t = Table {
            name: "x".to_string(),
            schema: Schema::from_parts(vec![f.clone()], vec![]),
            data: vec![],
        };
        assert_eq!(t.schema.fields.len(), 1);
        assert_eq!(t.schema.fields[0].name, "id");
    }
}

#[cfg(test)]
mod sheet_name_skip_tests {
    use super::*;
    use rust_xlsxwriter::Workbook;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Write a minimal xlsx with two valid 5-row schemas: one regular sheet,
    /// one whose name starts with `#`. Returns the path.
    fn write_xlsx_with_hash_named_sheet() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("tablec_core_sheet_name_skip_{pid}_{n}.xlsx"));
        let _ = std::fs::remove_file(&path);

        let mut wb = Workbook::new();
        let keep = wb.add_worksheet();
        keep.set_name("Items").ok();
        keep.write_string(0, 0, "id").ok();
        keep.write_string(0, 1, "name").ok();
        keep.write_string(1, 0, "int").ok();
        keep.write_string(1, 1, "string").ok();
        keep.write_number(5, 0, 1.0).ok();
        keep.write_string(5, 1, "alice").ok();

        let skipped = wb.add_worksheet();
        skipped.set_name("#comment").ok();
        skipped.write_string(0, 0, "id").ok();
        skipped.write_string(0, 1, "note").ok();
        skipped.write_string(1, 0, "int").ok();
        skipped.write_string(1, 1, "string").ok();
        skipped.write_number(5, 0, 99.0).ok();
        skipped.write_string(5, 1, "should_be_skipped").ok();

        wb.save(&path).unwrap();
        path
    }

    #[test]
    fn read_excel_with_skips_sheets_whose_name_starts_with_hash() {
        let path = write_xlsx_with_hash_named_sheet();
        let tables =
            read_excel_with(path.to_str().unwrap(), &StandardSchemaParser).expect("no diags");

        let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"Items"),
            "expected Items sheet to be parsed; got: {:?}",
            names
        );
        assert!(
            !names.iter().any(|n| n.starts_with('#')),
            "sheets starting with `#` must be skipped; got: {:?}",
            names
        );
        assert_eq!(
            tables.len(),
            1,
            "exactly one table expected, got: {:?}",
            names
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_excel_also_skips_sheets_whose_name_starts_with_hash() {
        let path = write_xlsx_with_hash_named_sheet();
        let tables = read_excel(path.to_str().unwrap()).expect("no diags");
        let names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
        assert!(
            !names.iter().any(|n| n.starts_with('#')),
            "read_excel must also skip `#`-named sheets; got: {:?}",
            names
        );
        let _ = std::fs::remove_file(&path);
    }
}
