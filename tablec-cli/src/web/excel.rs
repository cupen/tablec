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

#[cfg(test)]
mod tests {
    use super::*;

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
}
