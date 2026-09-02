//! Calamine-backed excel reader.
//!
//! The webui uses these helpers directly instead of going through
//! [`tablec_core::core::table::table::read_excel`], because the latter
//! returns `Err(diagnostics)` and **drops all successfully-parsed tables**
//! if any single cell fails (`src/core/table/table.rs:118-122`). That makes
//! it unusable for "preview" endpoints, where we want both the cells *and*
//! the diagnostics.
//!
//! Everything here is read-only and side-effect-free.

use std::path::Path;

use calamine::{Data, ExcelDateTime, Reader, open_workbook_auto};
use serde::Serialize;

use tablec_core::core::diagnostic::{Diagnostic, SourceLocation};
use tablec_core::core::parser::value_parser::parse_value;
use tablec_core::core::schema::{SchemaParseResult, SchemaParser, SchemaParserRegistry};
use tablec_core::core::table::constraint::Constraint;
use tablec_core::core::table::field::{Field, FieldType};
use tablec_core::core::table::value::Value;

/// Errors surfaced by [`list_sheets`] and [`preview_sheet`].
#[derive(Debug, thiserror::Error)]
pub enum ExcelError {
    #[error("io error reading '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("calamine error opening '{path}': {source}")]
    Open {
        path: String,
        #[source]
        source: calamine::Error,
    },
    #[error("sheet '{sheet}' not found in '{path}'")]
    SheetNotFound { path: String, sheet: String },
}

/// Lightweight summary of a workbook's sheets.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SheetInfo {
    pub name: String,
    /// Best-effort count of populated rows (0 if it couldn't be measured).
    pub row_count: Option<usize>,
    /// Best-effort count of columns (max non-empty cells in any row, capped
    /// at the first 100 rows sampled).
    pub col_count: Option<usize>,
}

/// Open `path` and return a summary of every sheet. Sheet names beginning
/// with `#` are filtered (matching `read_excel`'s behavior).
pub fn list_sheets(path: &Path) -> Result<Vec<SheetInfo>, ExcelError> {
    let path_str = path.display().to_string();
    let mut wb = open_workbook_auto(path).map_err(|e| ExcelError::Open {
        path: path_str.clone(),
        source: e,
    })?;

    let mut out = Vec::new();
    for name in wb.sheet_names() {
        if name.starts_with('#') {
            continue;
        }
        let (rows, cols) = match wb.worksheet_range(&name) {
            Ok(range) => {
                let rows = range.height();
                let cols = (0..rows.min(100))
                    .map(|r| range.rows().nth(r).map(|r| r.len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                (Some(rows), Some(cols))
            }
            Err(_) => (None, None),
        };
        out.push(SheetInfo {
            name,
            row_count: rows,
            col_count: cols,
        });
    }
    Ok(out)
}

/// A single cell, serialized as a JSON value.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum Cell {
    /// Empty cell.
    Null,
    /// Floating-point number (calamine normalizes all numbers to `f64`).
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// Text cell.
    Str(String),
    /// DateTime — rendered as ISO 8601 string.
    DateTime(String),
    /// Duration — rendered as ISO 8601 duration string.
    Duration(String),
    /// Anything calamine reports that we don't recognize.
    Other(String),
}

impl From<&Data> for Cell {
    fn from(d: &Data) -> Self {
        match d {
            Data::Empty => Cell::Null,
            Data::String(s) => Cell::Str(s.clone()),
            Data::Float(f) => Cell::Float(*f),
            Data::Int(i) => Cell::Float(*i as f64),
            Data::Bool(b) => Cell::Bool(*b),
            Data::DateTime(dt) => render_datetime(dt),
            Data::DateTimeIso(s) => Cell::DateTime(s.clone()),
            Data::DurationIso(s) => Cell::Duration(s.clone()),
            Data::Error(_) => Cell::Other("ERROR".to_string()),
        }
    }
}

fn render_datetime(dt: &ExcelDateTime) -> Cell {
    // Best-effort string rendering without bringing in chrono. Format the
    // raw `f64` value with a marker so the UI knows it's a date/duration.
    if dt.is_duration() {
        Cell::Duration(format!("excel_serial={}", dt.as_f64()))
    } else if dt.is_datetime() {
        Cell::DateTime(format!("excel_serial={}", dt.as_f64()))
    } else {
        // Fallback (defensive — should be covered by the two arms above).
        Cell::Other(format!("datetime={}", dt.as_f64()))
    }
}

/// A 2D grid of cells — what `/api/preview` returns.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Grid {
    pub sheet: String,
    pub rows: Vec<Vec<Cell>>,
}

/// Read up to `max_rows` rows from `sheet` and return them as JSON-friendly cells.
///
/// The header rows (first 5) are always included even if `max_rows` is small —
/// callers usually want the schema visible regardless of data-row cap.
pub fn preview_sheet(path: &Path, sheet: &str, max_rows: usize) -> Result<Grid, ExcelError> {
    let path_str = path.display().to_string();
    let mut wb = open_workbook_auto(path).map_err(|e| ExcelError::Open {
        path: path_str.clone(),
        source: e,
    })?;

    let range = wb
        .worksheet_range(sheet)
        .map_err(|_| ExcelError::SheetNotFound {
            path: path_str.clone(),
            sheet: sheet.to_string(),
        })?;

    let total_rows = range.height();
    let cap = max_rows.max(5).min(total_rows);
    let mut rows = Vec::with_capacity(cap);
    for r in 0..cap {
        let mut row = Vec::new();
        if let Some(cells) = range.rows().nth(r) {
            for c in cells {
                row.push(Cell::from(c));
            }
        }
        rows.push(row);
    }
    Ok(Grid {
        sheet: sheet.to_string(),
        rows,
    })
}

// =============================================================================
// Parsed preview — schema + per-cell typed validation.
//
// This is what the webui shows by default: the file *as tablec will see it
// during build*, not as raw bytes. The caller chooses the schema parser from
// a registry (so plugin parsers work too). For every data row, each cell is
// run through `parse_value` against the field's declared type; failures show
// up as `ParsedCell { error: Some(...) }`.
//
// This is intentionally lighter than a full `ConstraintValidator::validate_table`
// pass (which also checks @unique/@range across rows): preview runs only the
// per-cell type check. The dedicated `/api/check` endpoint does the full pass.
// =============================================================================

/// Summary footer for a parsed preview.
#[derive(Debug, Clone, Serialize)]
pub struct PreviewSummary {
    pub data_rows: usize,
    pub shown_rows: usize,
    pub total_rows: usize,
    pub error_count: usize,
    pub warning_count: usize,
}

/// Lightweight schema info returned to the UI — full Schema minus internals
/// it doesn't need.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedSchemaInfo {
    pub fields: Vec<Field>,
    pub constraints: Vec<Constraint>,
}

/// One typed data cell.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedCell {
    /// Original cell text (from calamine's `to_string()`).
    pub raw: String,
    /// Typed value as JSON. `None` for empty cells or parse failures.
    pub value: Option<serde_json::Value>,
    /// Per-cell parse error, if any.
    pub error: Option<String>,
    /// Display name of the column type (e.g. "int32", "string", "array<int>").
    pub type_name: String,
}

/// One data row + how many cells failed.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedRow {
    /// 0-indexed row number in the sheet (always >= data_start_row).
    pub row_index: usize,
    /// 1-indexed line number (for human display).
    pub line: usize,
    pub cells: Vec<ParsedCell>,
    pub error_count: usize,
}

/// Result of running the schema parser + per-cell validation on one sheet.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedPreview {
    pub sheet: String,
    /// Schema the parser decided on. `None` when the parser returned Skip
    /// (e.g. sheet name starts with `#`) or when the sheet has no header rows.
    pub schema: Option<ParsedSchemaInfo>,
    pub data_start_row: usize,
    pub total_rows: usize,
    pub shown_rows: usize,
    pub rows: Vec<ParsedRow>,
    pub diagnostics: Vec<Diagnostic>,
    pub summary: PreviewSummary,
}

/// Run schema + per-cell validation on `sheet` and return a [`ParsedPreview`].
///
/// `registry` is consulted to pick the parser by name. Pass `"standard"` for
/// the default 5-row layout. `max_rows` caps the number of *data rows*
/// (schema rows are always included in [`ParsedPreview::schema`]).
pub fn parsed_preview(
    path: &Path,
    sheet: &str,
    parser_name: &str,
    registry: &SchemaParserRegistry,
    max_rows: usize,
) -> Result<ParsedPreview, ParsedPreviewError> {
    let parser = registry.get(parser_name).ok_or_else(|| {
        let mut available = registry.parser_names();
        available.sort();
        ParsedPreviewError::UnknownParser {
            name: parser_name.to_string(),
            available,
        }
    })?;
    parsed_preview_with(path, sheet, parser.as_ref(), max_rows)
}

#[derive(Debug, thiserror::Error)]
pub enum ParsedPreviewError {
    #[error("unknown parser '{name}'; available: {available:?}")]
    UnknownParser {
        name: String,
        available: Vec<String>,
    },
    #[error(transparent)]
    Excel(#[from] ExcelError),
}

/// Same as [`parsed_preview`] but with a pre-resolved parser reference — handy
/// for the handler hot path where the registry is already in `WebuiState`.
pub fn parsed_preview_with(
    path: &Path,
    sheet: &str,
    parser: &dyn SchemaParser,
    max_rows: usize,
) -> Result<ParsedPreview, ParsedPreviewError> {
    let path_str = path.display().to_string();
    let mut wb = open_workbook_auto(path).map_err(|e| ExcelError::Open {
        path: path_str.clone(),
        source: e,
    })?;

    let range = wb
        .worksheet_range(sheet)
        .map_err(|_| ExcelError::SheetNotFound {
            path: path_str.clone(),
            sheet: sheet.to_string(),
        })?;

    let total_rows = range.height();
    let raw: Vec<Vec<String>> = range
        .rows()
        .map(|row| row.iter().map(|c| c.to_string()).collect())
        .collect();

    // Parse the schema (5-row layout for the standard parser; custom parsers
    // may use whatever they want). `parse_schema` returns Skip when the sheet
    // doesn't qualify (e.g. starts with `#`).
    let schema_result = match parser.parse_schema(sheet, &raw) {
        Ok(r) => r,
        Err(diags) => {
            return Ok(ParsedPreview {
                sheet: sheet.to_string(),
                schema: None,
                data_start_row: 0,
                total_rows,
                shown_rows: 0,
                rows: Vec::new(),
                diagnostics: diags,
                summary: empty_summary(total_rows),
            });
        }
    };

    let schema = match schema_result {
        SchemaParseResult::Skip => {
            return Ok(ParsedPreview {
                sheet: sheet.to_string(),
                schema: None,
                data_start_row: 0,
                total_rows,
                shown_rows: 0,
                rows: Vec::new(),
                diagnostics: Vec::new(),
                summary: empty_summary(total_rows),
            });
        }
        SchemaParseResult::Schema(s) => s,
    };

    let schema_info = ParsedSchemaInfo {
        fields: schema.fields.clone(),
        constraints: schema.constraints.clone(),
    };
    let data_start = schema.data_start_row;

    // Cap data rows. Cap is at least 1 so the user sees something; 0 means
    // "the sheet has no data" which the empty-state should reflect.
    let data_rows_total = total_rows.saturating_sub(data_start);
    let cap = max_rows.max(1).min(data_rows_total);

    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut parsed_rows: Vec<ParsedRow> = Vec::with_capacity(cap);
    let mut total_errors = 0usize;

    for (offset, row) in raw.iter().enumerate().skip(data_start).take(cap) {
        let mut cells = Vec::with_capacity(schema.fields.len());
        let mut errs = 0usize;
        for (col_idx, field) in schema.fields.iter().enumerate() {
            let raw_cell = row.get(col_idx).cloned().unwrap_or_default();
            let loc = SourceLocation {
                file: Some(path.to_path_buf()),
                sheet: Some(sheet.to_string()),
                line: Some((offset + 1) as u32),
                column: Some((col_idx + 1) as u32),
            };
            let (value, error) = if raw_cell.is_empty() {
                (None, None)
            } else {
                match parse_value(&raw_cell, &field.t, loc) {
                    Ok(v) => (Some(value_to_json(&v)), None),
                    Err(d) => {
                        errs += 1;
                        diagnostics.push(d);
                        (
                            None,
                            Some(format!(
                                "type mismatch: expected {}",
                                field_type_name(&field.t)
                            )),
                        )
                    }
                }
            };
            cells.push(ParsedCell {
                raw: raw_cell,
                value,
                error,
                type_name: field_type_name(&field.t).to_string(),
            });
        }
        total_errors += errs;
        parsed_rows.push(ParsedRow {
            row_index: offset,
            line: offset + 1,
            cells,
            error_count: errs,
        });
    }

    let warn_count = diagnostics
        .iter()
        .filter(|d| format!("{:?}", d.severity).contains("Warning"))
        .count();

    Ok(ParsedPreview {
        sheet: sheet.to_string(),
        schema: Some(schema_info),
        data_start_row: data_start,
        total_rows,
        shown_rows: cap,
        rows: parsed_rows,
        diagnostics,
        summary: PreviewSummary {
            data_rows: data_rows_total,
            shown_rows: cap,
            total_rows,
            error_count: total_errors,
            warning_count: warn_count,
        },
    })
}

fn empty_summary(total_rows: usize) -> PreviewSummary {
    PreviewSummary {
        data_rows: 0,
        shown_rows: 0,
        total_rows,
        error_count: 0,
        warning_count: 0,
    }
}

/// Short human-readable name for a `FieldType` (e.g. "int32", "string",
/// "array<int>"). Used by the UI to label column types in the schema row.
fn field_type_name(t: &FieldType) -> &'static str {
    match t {
        FieldType::Int => "int",
        FieldType::Int8 => "int8",
        FieldType::Int16 => "int16",
        FieldType::Int32 => "int32",
        FieldType::Int64 => "int64",
        FieldType::Uint => "uint",
        FieldType::Uint8 => "uint8",
        FieldType::Uint16 => "uint16",
        FieldType::Uint32 => "uint32",
        FieldType::Uint64 => "uint64",
        FieldType::Float => "float",
        FieldType::Float32 => "float32",
        FieldType::Float64 => "float64",
        FieldType::String => "string",
        FieldType::Bool => "bool",
        FieldType::Date => "date",
        FieldType::DateTime => "datetime",
        FieldType::Timestamp32 => "timestamp32",
        FieldType::Timestamp64 => "timestamp64",
        FieldType::Array { .. } => "array",
        FieldType::Map { .. } => "map",
        FieldType::Struct { .. } => "struct",
    }
}

/// Serialize a `Value` to a JSON value for the webui. Reuses the existing
/// `Value: Serialize` impl via a tiny `serde_json::Value` shim.
fn value_to_json(v: &Value) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablec_core::core::schema::SchemaParserRegistry;

    /// Resolve a fixture file shipped by tablec-core.
    fn fixture(rel: &str) -> std::path::PathBuf {
        // tablec-cli depends on tablec-core by path; the fixtures live under
        // tablec-core/tests/fixtures/.
        let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        crate_dir.join("../tablec-core/tests/fixtures").join(rel)
    }

    #[test]
    fn list_sheets_filters_hash_and_returns_counts() {
        let p = fixture("testdata/basic_table.xlsx");
        let sheets = list_sheets(&p).expect("open fixture");
        assert!(!sheets.is_empty());
        assert!(
            sheets.iter().all(|s| !s.name.starts_with('#')),
            "hash-prefixed sheets should be filtered, got {:?}",
            sheets.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        for s in &sheets {
            assert!(s.row_count.unwrap_or(0) > 0, "{:?} empty?", s.name);
        }
    }

    #[test]
    fn preview_sheet_returns_at_least_five_rows() {
        let p = fixture("testdata/basic_table.xlsx");
        let sheets = list_sheets(&p).expect("open");
        let target = sheets.first().expect("at least one sheet").name.clone();
        let grid = preview_sheet(&p, &target, 3).expect("preview");
        assert_eq!(grid.sheet, target);
        // 5 schema rows must always be present, even when max_rows=3
        assert!(grid.rows.len() >= 5, "got {} rows", grid.rows.len());
        assert!(!grid.rows[0].is_empty(), "header row shouldn't be empty");
    }

    #[test]
    fn preview_unknown_sheet_returns_error() {
        let p = fixture("testdata/basic_table.xlsx");
        let err = preview_sheet(&p, "does-not-exist", 10).unwrap_err();
        match err {
            ExcelError::SheetNotFound { sheet, .. } => {
                assert_eq!(sheet, "does-not-exist");
            }
            other => panic!("expected SheetNotFound, got {other:?}"),
        }
    }

    fn registry() -> SchemaParserRegistry {
        SchemaParserRegistry::with_standard()
    }

    #[test]
    fn parsed_preview_returns_schema_for_basic_fixture() {
        let p = fixture("testdata/basic_table.xlsx");
        if !p.exists() {
            eprintln!("skipping: fixture {} not present", p.display());
            return;
        }
        let sheets = list_sheets(&p).expect("open");
        let target = sheets.first().expect("at least one sheet").name.clone();
        let reg = registry();
        let pp = parsed_preview(&p, &target, "standard", &reg, 50).expect("parsed");
        assert_eq!(pp.sheet, target);
        let schema = pp.schema.expect("schema should be present");
        assert!(
            !schema.fields.is_empty(),
            "expected at least one parsed field"
        );
        assert_eq!(pp.data_start_row, 5);
        assert!(pp.summary.total_rows >= 5);
        // Rows are typed: at least one cell should have a typed value, and the
        // basic fixture is well-formed so error_count stays 0.
        assert!(
            pp.summary.error_count == 0,
            "unexpected errors: {:?}",
            pp.diagnostics
        );
        assert!(!pp.rows.is_empty(), "expected some data rows");
        assert!(
            pp.rows[0].cells.iter().any(|c| c.value.is_some()),
            "expected at least one typed cell in row 0"
        );
        assert!(
            pp.rows[0].cells.iter().all(|c| !c.type_name.is_empty()),
            "type_name must be populated for every cell"
        );
    }

    #[test]
    fn parsed_preview_unknown_parser_returns_error() {
        let p = fixture("testdata/basic_table.xlsx");
        let sheets = list_sheets(&p).expect("open");
        let target = sheets.first().unwrap().name.clone();
        let reg = registry();
        let err = parsed_preview(&p, &target, "does-not-exist", &reg, 10).unwrap_err();
        match err {
            ParsedPreviewError::UnknownParser { name, available } => {
                assert_eq!(name, "does-not-exist");
                assert!(available.contains(&"standard".to_string()));
            }
            other => panic!("expected UnknownParser, got {other:?}"),
        }
    }

    #[test]
    fn parsed_preview_unknown_sheet_returns_error() {
        let p = fixture("testdata/basic_table.xlsx");
        let reg = registry();
        let err = parsed_preview(&p, "does-not-exist", "standard", &reg, 10).unwrap_err();
        match err {
            ParsedPreviewError::Excel(ExcelError::SheetNotFound { sheet, .. }) => {
                assert_eq!(sheet, "does-not-exist");
            }
            other => panic!("expected SheetNotFound, got {other:?}"),
        }
    }

    #[test]
    fn field_type_name_returns_nonempty_for_every_variant() {
        // Spot-check that every FieldType variant maps to a non-empty label.
        let cases: Vec<FieldType> = vec![
            FieldType::Int,
            FieldType::Int32,
            FieldType::Float64,
            FieldType::String,
            FieldType::Bool,
            FieldType::Array {
                r#type: Box::new(FieldType::Int32),
            },
            FieldType::Map {
                key: Box::new(FieldType::String),
                value: Box::new(FieldType::Int32),
            },
            FieldType::Struct { fields: vec![] },
        ];
        for c in cases {
            assert!(!field_type_name(&c).is_empty(), "missing name for {c:?}");
        }
    }
}
