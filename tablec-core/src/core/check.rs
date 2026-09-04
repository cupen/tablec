//! Shared data-check pipeline — the single check entry point used by both
//! the CLI (`tablec check`) and the webui (`POST /api/check`).
//!
//! Pipeline: enumerate files with the configured include/exclude globs
//! ([`find_excel_files`]) → parse each file with the selected schema parser
//! ([`read_excel_with`]) → validate per table, then run project-level
//! validation exactly once over the complete table set.
//!
//! The per-table and cross-table layers are both provided by a single
//! [`ConstraintValidator::validate_project`] call (its first pass runs
//! `validate_table` on every table, its second pass resolves `@ref` /
//! `@no_ref` across the full set). Calling `validate_table` separately as
//! well would duplicate every per-table diagnostic.
//!
//! Diagnostics ordering: parse diagnostics (file order) → per-table
//! diagnostics (table order) → project/cross-table diagnostics (table order).

use crate::core::config::find_excel_files;
use crate::core::diagnostic::{Diagnostic, DiagnosticCode, Severity, SourceLocation};
use crate::core::schema::SchemaParser;
use crate::core::table::constraint::ConstraintValidator;
use crate::core::table::table::{Table, read_excel_with};

/// Result of a full check run over an input directory.
#[derive(Debug)]
pub struct CheckOutcome {
    /// Every table that parsed successfully, in file order then sheet order.
    pub tables: Vec<Table>,
    /// All diagnostics: parse errors (file order) → per-table (table order)
    /// → project (table order).
    pub diagnostics: Vec<Diagnostic>,
}

/// Run the shared check pipeline over `input_dir`.
///
/// A directory that contains no matching files is not an error: the returned
/// outcome carries a single warning diagnostic naming the directory and its
/// include/exclude patterns (and no tables). `Err` is reserved for failures
/// while enumerating the directory itself (e.g. an invalid glob pattern).
pub fn check_project(
    input_dir: &str,
    include: &[String],
    exclude: &[String],
    parser: &dyn SchemaParser,
) -> Result<CheckOutcome, Box<dyn std::error::Error>> {
    let files = find_excel_files(input_dir, include, exclude)?;

    let mut outcome = CheckOutcome {
        tables: Vec::new(),
        diagnostics: Vec::new(),
    };

    if files.is_empty() {
        outcome.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: DiagnosticCode::Other,
            message: format!(
                "no spreadsheet files found under {} (include: {:?}, exclude: {:?})",
                input_dir, include, exclude
            ),
            location: SourceLocation::default(),
        });
        return Ok(outcome);
    }

    // Parse every file with the selected parser, accumulating parse
    // diagnostics in file order.
    for file in &files {
        match read_excel_with(&file.to_string_lossy(), parser) {
            Ok(mut tables) => outcome.tables.append(&mut tables),
            Err(errs) => outcome.diagnostics.extend(errs),
        }
    }

    // One project-level validation over the complete table set: per-table
    // constraints first, then cross-table `@ref`/`@no_ref` — see the module
    // docs for why this is a single call.
    if let Err(errs) = ConstraintValidator::validate_project(&outcome.tables) {
        outcome.diagnostics.extend(errs);
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diagnostic::DiagnosticCode;
    use crate::core::schema::{SchemaParseResult, StandardSchemaParser};
    use rust_xlsxwriter::Workbook;
    use std::path::{Path, PathBuf};

    // ---------------------------------------------------------------------
    // xlsx fixture helpers (standard 5-row tablec layout)
    // ---------------------------------------------------------------------

    /// Add a sheet with the standard 5-row layout: field names, types,
    /// comments, field constraints, table constraints (empty), then data
    /// rows starting at index 5.
    fn add_sheet(
        wb: &mut Workbook,
        name: &str,
        columns: &[(&str, &str, &str)], // (name, type, constraint)
        data: &[&[&str]],
    ) {
        let sheet = wb.add_worksheet();
        sheet.set_name(name).ok();
        for (col, (field_name, ty, constraint)) in columns.iter().enumerate() {
            let col = col as u16;
            sheet.write_string(0, col, *field_name).ok();
            sheet.write_string(1, col, *ty).ok();
            sheet.write_string(2, col, "").ok();
            sheet.write_string(3, col, *constraint).ok();
            sheet.write_string(4, col, "").ok();
        }
        for (row, cells) in data.iter().enumerate() {
            for (col, cell) in cells.iter().enumerate() {
                sheet.write_string(5 + row as u32, col as u16, *cell).ok();
            }
        }
    }

    fn save(mut wb: Workbook, dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        wb.save(&path).unwrap();
        path
    }

    /// Target table `Item` (column `id`) with the given id values.
    fn write_items(dir: &Path, name: &str, ids: &[&str]) -> PathBuf {
        let mut wb = Workbook::new();
        let rows: Vec<&[&str]> = ids.iter().map(std::slice::from_ref).collect();
        add_sheet(&mut wb, "Item", &[("id", "int", "")], &rows);
        save(wb, dir, name)
    }

    /// Host table `Drop` (column `item_id`) holding `@ref("Item.id")`.
    fn write_drop(dir: &Path, name: &str, item_ids: &[&str]) -> PathBuf {
        let mut wb = Workbook::new();
        let rows: Vec<&[&str]> = item_ids.iter().map(std::slice::from_ref).collect();
        add_sheet(
            &mut wb,
            "Drop",
            &[("item_id", "int", "@ref(\"Item.id\")")],
            &rows,
        );
        save(wb, dir, name)
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn count_code(diags: &[Diagnostic], code: DiagnosticCode) -> usize {
        diags.iter().filter(|d| d.code == code).count()
    }

    fn std_check(dir: &Path) -> CheckOutcome {
        check_project(
            &dir.to_string_lossy(),
            &strs(&["*.xlsx"]),
            &[],
            &StandardSchemaParser,
        )
        .expect("enumeration succeeds")
    }

    // ---------------------------------------------------------------------
    // Cross-file @ref: clean only with the full table set
    // ---------------------------------------------------------------------

    #[test]
    fn cross_file_ref_validates_clean_only_with_full_set() {
        let tmp = tempfile::tempdir().unwrap();
        let _items = write_items(tmp.path(), "b_items.xlsx", &["1", "2"]);
        let _drop = write_drop(tmp.path(), "a_drop.xlsx", &["1", "2"]);

        let outcome = std_check(tmp.path());
        assert!(
            outcome.diagnostics.is_empty(),
            "expected clean check with the full set, got: {:?}",
            outcome.diagnostics
        );
        assert_eq!(outcome.tables.len(), 2, "both sheets parsed");
        let names: Vec<&str> = outcome.tables.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Item") && names.contains(&"Drop"));

        // Prove the cross-file link is what makes it clean: with only the
        // host-side file absent, the very same Drop table violates @ref.
        let only_host_side = tempfile::tempdir().unwrap();
        let _drop = write_drop(only_host_side.path(), "a_drop.xlsx", &["1", "2"]);
        let outcome = std_check(only_host_side.path());
        assert_eq!(
            count_code(
                &outcome.diagnostics,
                DiagnosticCode::ConstraintForeignKeyViolation
            ),
            1,
            "expected exactly the missing-target violation, got: {:?}",
            outcome.diagnostics
        );
    }

    // ---------------------------------------------------------------------
    // Project violations appear exactly once
    // ---------------------------------------------------------------------

    #[test]
    fn project_violation_reported_exactly_once() {
        let tmp = tempfile::tempdir().unwrap();
        // Three files: the violating host table first (alphabetical), the
        // target in the second file, plus an unrelated third file. The old
        // incremental validate_project loop duplicated the violation once per
        // additional file; the shared pipeline must report it exactly once.
        let _drop = write_drop(tmp.path(), "a_drop.xlsx", &["99"]);
        let _items = write_items(tmp.path(), "b_items.xlsx", &["1"]);
        let mut wb = Workbook::new();
        add_sheet(&mut wb, "Misc", &[("note", "string", "")], &[&["hi"]]);
        let _misc = save(wb, tmp.path(), "c_misc.xlsx");

        let outcome = std_check(tmp.path());
        assert_eq!(
            count_code(
                &outcome.diagnostics,
                DiagnosticCode::ConstraintForeignKeyViolation
            ),
            1,
            "cross-file violation must appear exactly once, got: {:?}",
            outcome.diagnostics
        );
        // Pin the failure mode: the host value is missing from the (found)
        // target column — not a missing-target-table error.
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("missing from target Item.id"),
            "expected a FK violation against the found target, got: {:?}",
            outcome.diagnostics
        );
        assert_eq!(outcome.tables.len(), 3, "all three sheets parsed");
    }

    // ---------------------------------------------------------------------
    // Parse diagnostics accumulate across files
    // ---------------------------------------------------------------------

    #[test]
    fn parse_diagnostics_accumulate_across_files() {
        let tmp = tempfile::tempdir().unwrap();
        // Each file has an int8 field with an out-of-range value → a parse
        // diagnostic per file (read_excel_with is all-or-nothing per file).
        let mut a = Workbook::new();
        add_sheet(&mut a, "BadA", &[("n", "int8", "")], &[&["200"]]);
        let _a = save(a, tmp.path(), "a_bad.xlsx");
        let mut b = Workbook::new();
        add_sheet(&mut b, "BadB", &[("n", "int8", "")], &[&["300"]]);
        let _b = save(b, tmp.path(), "b_bad.xlsx");

        let outcome = std_check(tmp.path());
        assert!(
            outcome.tables.is_empty(),
            "no tables survive a failed parse: {:?}",
            outcome.tables
        );
        for name in ["a_bad.xlsx", "b_bad.xlsx"] {
            let hits = outcome
                .diagnostics
                .iter()
                .filter(|d| {
                    d.code == DiagnosticCode::ValueOutOfRange
                        && d.location
                            .file
                            .as_deref()
                            .is_some_and(|f| f.to_string_lossy().ends_with(name))
                })
                .count();
            assert_eq!(hits, 1, "expected one parse diag from {name}");
        }
    }

    // ---------------------------------------------------------------------
    // Parser selection is honored
    // ---------------------------------------------------------------------

    /// A parser that accepts only the sheet named `accept` (delegating to
    /// the standard layout) and skips everything else.
    struct OnlySheetParser {
        accept: &'static str,
    }

    impl SchemaParser for OnlySheetParser {
        fn name(&self) -> &str {
            "only-sheet"
        }
        fn parse_schema(
            &self,
            sheet_name: &str,
            sheet: &[Vec<String>],
        ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
            if sheet_name == self.accept {
                StandardSchemaParser.parse_schema(sheet_name, sheet)
            } else {
                Ok(SchemaParseResult::Skip)
            }
        }
    }

    #[test]
    fn parser_selection_is_honored() {
        let tmp = tempfile::tempdir().unwrap();
        let mut wb = Workbook::new();
        add_sheet(&mut wb, "Alpha", &[("id", "int", "")], &[&["1"]]);
        let _a = save(wb, tmp.path(), "one.xlsx");

        // Standard parser parses the sheet…
        let outcome = std_check(tmp.path());
        assert_eq!(outcome.tables.len(), 1);
        assert!(outcome.diagnostics.is_empty());

        // …the selected parser that skips "Alpha" yields no tables…
        let outcome = check_project(
            &tmp.path().to_string_lossy(),
            &strs(&["*.xlsx"]),
            &[],
            &OnlySheetParser { accept: "Beta" },
        )
        .unwrap();
        assert!(
            outcome.tables.is_empty(),
            "skipped sheet must not be parsed: {:?}",
            outcome.tables
        );
        assert!(outcome.diagnostics.is_empty());

        // …and the same parser accepting "Alpha" parses it again.
        let outcome = check_project(
            &tmp.path().to_string_lossy(),
            &strs(&["*.xlsx"]),
            &[],
            &OnlySheetParser { accept: "Alpha" },
        )
        .unwrap();
        assert_eq!(outcome.tables.len(), 1);
        assert_eq!(outcome.tables[0].name, "Alpha");
    }

    // ---------------------------------------------------------------------
    // Empty file set → warning diagnostic
    // ---------------------------------------------------------------------

    #[test]
    fn empty_dir_yields_warning_diagnostic() {
        let tmp = tempfile::tempdir().unwrap();
        let outcome = std_check(tmp.path());
        assert_eq!(outcome.tables.len(), 0);
        assert_eq!(outcome.diagnostics.len(), 1, "only the warning expected");
        let w = &outcome.diagnostics[0];
        assert_eq!(w.severity, Severity::Warning);
        assert_eq!(w.code, DiagnosticCode::Other);
        assert!(
            w.message.contains(tmp.path().to_string_lossy().as_ref()),
            "warning must name the input dir: {:?}",
            w.message
        );
        assert!(
            w.message.contains(r#"["*.xlsx"]"#),
            "warning must name the include patterns: {:?}",
            w.message
        );
    }

    #[test]
    fn nonexistent_input_dir_yields_the_same_warning() {
        // find_excel_files treats a missing directory as an empty set, so the
        // shared pipeline responds with the same warning diagnostic.
        let missing = std::env::temp_dir().join("tablec_check_no_such_dir_xyz");
        let _ = std::fs::remove_dir_all(&missing);
        let outcome = check_project(
            &missing.to_string_lossy(),
            &strs(&["*.xlsx"]),
            &[],
            &StandardSchemaParser,
        )
        .unwrap();
        assert!(outcome.tables.is_empty());
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(outcome.diagnostics[0].severity, Severity::Warning);
    }

    // ---------------------------------------------------------------------
    // Enumeration failures surface as Err
    // ---------------------------------------------------------------------

    #[test]
    fn invalid_glob_pattern_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let r = check_project(
            &tmp.path().to_string_lossy(),
            &strs(&["["]), // unclosed character class → glob PatternError
            &[],
            &StandardSchemaParser,
        );
        assert!(r.is_err(), "invalid include glob must be an Err");
    }

    // ---------------------------------------------------------------------
    // Diagnostic ordering: parse → per-table → project
    // ---------------------------------------------------------------------

    #[test]
    fn diagnostics_ordered_parse_then_table_then_project() {
        let tmp = tempfile::tempdir().unwrap();
        // Parse diag: int8 out of range.
        let mut a = Workbook::new();
        add_sheet(&mut a, "BadParse", &[("n", "int8", "")], &[&["200"]]);
        let _a = save(a, tmp.path(), "a_parse.xlsx");
        // Per-table diag: duplicated @unique values.
        let mut b = Workbook::new();
        add_sheet(
            &mut b,
            "Dup",
            &[("id", "int", "@unique")],
            &[&["1"], &["1"]],
        );
        let _b = save(b, tmp.path(), "b_table.xlsx");
        // Project diag: @ref target table does not exist.
        let mut c = Workbook::new();
        add_sheet(
            &mut c,
            "Ref",
            &[("item_id", "int", "@ref(\"NoSuch.id\")")],
            &[&["1"]],
        );
        let _c = save(c, tmp.path(), "c_ref.xlsx");

        let outcome = std_check(tmp.path());
        let pos = |code: DiagnosticCode| {
            outcome
                .diagnostics
                .iter()
                .position(|d| d.code == code)
                .unwrap_or_else(|| panic!("missing {code:?} in {:?}", outcome.diagnostics))
        };
        let parse = pos(DiagnosticCode::ValueOutOfRange);
        let table = pos(DiagnosticCode::ConstraintDuplicate);
        let project = pos(DiagnosticCode::ConstraintForeignKeyViolation);
        assert!(
            parse < table && table < project,
            "expected parse < per-table < project, got parse={parse} table={table} project={project}: {:?}",
            outcome.diagnostics
        );
    }
}
