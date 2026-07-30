//! 示例 plugin：8 行头布局
//!
//! 期望布局：
//! - row 0,1: 跳过
//! - row 2: 装饰（中文表名之类，跳过）
//! - row 3: 字段名
//! - row 4: 字段类型
//! - row 5: 字段注释
//! - row 6: 字段约束
//! - row 7: 表约束
//! - row 8+: 数据
//!
//! 借用 StandardSchemaParser 内部 helper（assemble_fields / parse_table_constraints）
//! 不重复实现 type / constraint 解析逻辑。

#![cfg(any(test, doc))]

use crate::core::diagnostic::Diagnostic;
use crate::core::schema::{Schema, SchemaParseResult, SchemaParser};
use crate::core::table::constraint::Constraint;
use crate::core::table::field::Field;

pub struct EightRowHeaderParser;

impl SchemaParser for EightRowHeaderParser {
    fn name(&self) -> &str {
        "eight-row"
    }

    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
        if sheet.len() < 8 {
            return Err(vec![Diagnostic::new(
                crate::core::diagnostic::DiagnosticCode::HeaderParserError,
                format!("eight-row requires at least 8 rows, got {}", sheet.len()),
                Default::default(),
            )]);
        }

        // 复用 StandardSchemaParser 的字段装配逻辑（标准实现已经处理类型 fallback / 标签切分）
        let std = crate::core::schema::StandardSchemaParser;
        let fields: Vec<Field> = {
            // 把 row 3..7 当作"标准 5 行布局"喂给 StandardSchemaParser
            let five_row: Vec<Vec<String>> = vec![
                sheet[3].clone(),
                sheet[4].clone(),
                sheet[5].clone(),
                sheet[6].clone(),
                sheet[7].clone(),
            ];
            match std.parse_schema(sheet_name, &five_row)? {
                SchemaParseResult::Schema(s) => s.fields,
                SchemaParseResult::Skip => return Ok(SchemaParseResult::Skip),
            }
        };

        let constraints = {
            // row 7 是 table constraints
            let mut out = Vec::new();
            for raw in sheet[7].iter() {
                let cell = raw.trim();
                if cell.is_empty() {
                    continue;
                }
                let c =
                    Constraint::from_str_with_loc(cell, Default::default()).map_err(|d| vec![d])?;
                out.push(c);
            }
            out
        };

        Ok(SchemaParseResult::Schema(Schema {
            fields,
            constraints,
            data_start_row: 8,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::FieldType;

    fn sheet_with_rows(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn name_returns_eight_row() {
        assert_eq!(EightRowHeaderParser.name(), "eight-row");
    }

    #[test]
    fn parses_8_row_layout() {
        let sheet = sheet_with_rows(&[
            &[""],
            &[""],
            &[""],              // row 0,1,2 跳过
            &["id", "name"],    // row 3 字段名
            &["int", "string"], // row 4 字段类型
            &["ID", "Name"],    // row 5 注释
            &["", ""],          // row 6 字段约束
            &[""],              // row 7 表约束
            &["1", "alice"],    // row 8 data
            &["2", "bob"],
        ]);
        let p = EightRowHeaderParser;
        let r = p.parse_schema("T", &sheet).unwrap();
        match r {
            SchemaParseResult::Schema(s) => {
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name, "id");
                assert_eq!(s.fields[0].t, FieldType::Int32);
                assert_eq!(s.fields[1].name, "name");
                assert_eq!(s.fields[1].t, FieldType::String);
                assert_eq!(s.data_start_row, 8);
            }
            _ => panic!("expected Schema"),
        }
    }

    #[test]
    fn short_sheet_yields_error() {
        let sheet = sheet_with_rows(&[&["a"], &["b"]]);
        let p = EightRowHeaderParser;
        let r = p.parse_schema("T", &sheet);
        assert!(r.is_err());
    }
}
