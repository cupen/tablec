//! Validation report generation — one [`Report`] value describing a check
//! run, rendered by the JSON ([`json`]) and HTML ([`html`]) renderers.
//!
//! The report is built once from a [`CheckOutcome`] plus caller-supplied
//! metadata and consumed by both renderers, so every surface (CLI today,
//! webui export / Python bindings later) produces identical artifacts.
//!
//! The JSON envelope's top-level member set is locked by a structure test;
//! any change that breaks consumers must bump [`REPORT_SCHEMA_VERSION`].

mod html;
mod json;

pub use json::REPORT_SCHEMA_VERSION;

use crate::core::check::CheckOutcome;
use crate::core::diagnostic::{Diagnostic, Severity};
use serde::Serialize;

/// A validation report describing one check run.
///
/// Built once per run via [`Report::from_outcome`] and consumed by both
/// renderers ([`Report::render_json`] and [`Report::render_html`]).
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Envelope schema version — pins the JSON contract (see the spec).
    pub schema_version: u32,
    /// Version of the tool that produced the report (supplied by the caller).
    pub tablec_version: String,
    /// Generation time, RFC 3339 UTC (e.g. `2026-10-27T12:34:56Z`).
    pub generated_at: String,
    /// Directory the check scanned.
    pub input_dir: String,
    /// Schema parser used by the check run.
    pub parser: String,
    /// Severity counts and file/sheet census for the run.
    pub summary: ReportSummary,
    /// Every diagnostic the pipeline produced, in pipeline order.
    pub diagnostics: Vec<Diagnostic>,
    /// Names of sheets that parsed and carry no diagnostic. Feeds the HTML
    /// report's passed list only — never serialized (the JSON v1 envelope
    /// member set is locked by the structure test).
    #[serde(skip_serializing)]
    pub passed_sheets: Vec<String>,
}

/// Summary counters carried by the report envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ReportSummary {
    /// Diagnostics with error severity.
    pub errors: usize,
    /// Diagnostics with warning severity.
    pub warnings: usize,
    /// Distinct spreadsheet files the pipeline enumerated and processed.
    /// A file that parsed cleanly counts exactly like one that failed;
    /// see [`CheckOutcome::files`] for the census this is taken from.
    pub files_checked: usize,
    /// Number of tables (sheets) that were checked.
    pub tables_checked: usize,
}

impl Report {
    /// Build a report from a check run's outcome. `tablec_version` is
    /// supplied by the caller (the producing binary's own version);
    /// `generated_at` is the wall-clock time at build.
    pub fn from_outcome(
        outcome: &CheckOutcome,
        tablec_version: &str,
        input_dir: &str,
        parser: &str,
    ) -> Report {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        Self::from_outcome_at(outcome, tablec_version, input_dir, parser, secs)
    }

    /// [`Report::from_outcome`] at a pinned epoch instant (deterministic
    /// tests).
    pub(crate) fn from_outcome_at(
        outcome: &CheckOutcome,
        tablec_version: &str,
        input_dir: &str,
        parser: &str,
        unix_secs: u64,
    ) -> Report {
        Report {
            schema_version: REPORT_SCHEMA_VERSION,
            tablec_version: tablec_version.to_string(),
            generated_at: format_rfc3339_utc(unix_secs),
            input_dir: input_dir.to_string(),
            parser: parser.to_string(),
            summary: ReportSummary {
                errors: count_severity(&outcome.diagnostics, Severity::Error),
                warnings: count_severity(&outcome.diagnostics, Severity::Warning),
                files_checked: files_checked(outcome),
                tables_checked: outcome.tables.len(),
            },
            diagnostics: outcome.diagnostics.clone(),
            passed_sheets: passed_sheets(outcome),
        }
    }
}

fn count_severity(diags: &[Diagnostic], severity: Severity) -> usize {
    diags
        .iter()
        .filter(|d| matches!(d.severity, s if s == severity))
        .count()
}

/// Distinct non-empty paths in the outcome's `files` census — the files the
/// pipeline enumerated and processed, cleanly parsed or not.
fn files_checked(outcome: &CheckOutcome) -> usize {
    let mut seen: Vec<&std::path::Path> = Vec::new();
    for f in &outcome.files {
        if f.as_os_str().is_empty() || seen.contains(&f.as_path()) {
            continue;
        }
        seen.push(f);
    }
    seen.len()
}

/// Sheets that parsed and carry no diagnostic, in table order. A table is
/// "passed" when no diagnostic in the outcome references its sheet name —
/// per-table violations surface as diagnostics naming the violating sheet.
fn passed_sheets(outcome: &CheckOutcome) -> Vec<String> {
    outcome
        .tables
        .iter()
        .filter(|t| {
            !outcome
                .diagnostics
                .iter()
                .any(|d| d.location.sheet.as_deref() == Some(t.name.as_str()))
        })
        .map(|t| t.name.clone())
        .collect()
}

/// Format `unix_secs` (seconds since the Unix epoch) as an RFC 3339 UTC
/// timestamp, e.g. `2026-10-27T12:34:56Z`. Hand-rolled civil-from-days
/// conversion (Howard Hinnant's algorithm) — tablec-core carries no time
/// crate, and one timestamp does not justify adding one.
pub(crate) fn format_rfc3339_utc(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let sod = unix_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

/// Days since 1970-01-01 → `(year, month, day)` in the proleptic Gregorian
/// calendar. Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (y + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diagnostic::DiagnosticCode;
    use crate::core::diagnostic::SourceLocation;

    fn diag(sev: Severity, msg: &str, file: Option<&str>, sheet: Option<&str>) -> Diagnostic {
        Diagnostic {
            severity: sev,
            code: DiagnosticCode::Other,
            message: msg.to_string(),
            location: SourceLocation {
                file: file.map(std::path::PathBuf::from),
                sheet: sheet.map(str::to_string),
                line: None,
                column: None,
            },
        }
    }

    fn outcome(files: &[&str], tables: &[&str], diags: &[Diagnostic]) -> CheckOutcome {
        CheckOutcome {
            tables: tables
                .iter()
                .map(|name| crate::core::table::table::Table {
                    name: (*name).to_string(),
                    schema: crate::core::schema::Schema::from_parts(vec![], vec![]),
                    data: vec![],
                })
                .collect(),
            diagnostics: diags.to_vec(),
            files: files.iter().map(std::path::PathBuf::from).collect(),
        }
    }

    // -------------------------------------------------------------------
    // 1.1 Summary counts
    // -------------------------------------------------------------------

    #[test]
    fn summary_counts_by_severity() {
        let o = outcome(
            &["f1.xlsx"],
            &["A"],
            &[
                diag(Severity::Error, "e1", Some("f1.xlsx"), Some("A")),
                diag(Severity::Error, "e2", None, None),
                diag(Severity::Warning, "w1", None, None),
            ],
        );
        let r = Report::from_outcome(&o, "1.2.3", "dir", "standard");
        assert_eq!(r.summary.errors, 2);
        assert_eq!(r.summary.warnings, 1);
        assert_eq!(r.summary.tables_checked, 1);
    }

    #[test]
    fn files_checked_counts_enumerated_files_distinctly() {
        // The census is the outcome's file list: f1 appears twice (dup) and
        // counts once; each parsed or failing file counts equally.
        let o = outcome(&["f1.xlsx", "f2.xlsx", "f1.xlsx"], &["A", "B"], &[]);
        let r = Report::from_outcome(&o, "v", "dir", "standard");
        assert_eq!(r.summary.files_checked, 2);
    }

    #[test]
    fn clean_run_counts_files_even_without_diagnostics() {
        // Regression: a fully clean run checks real files — the census must
        // not depend on diagnostics carrying file locations.
        let o = outcome(&["a.xlsx", "b.xlsx"], &["A", "B"], &[]);
        let r = Report::from_outcome(&o, "v", "dir", "standard");
        assert_eq!(r.summary.files_checked, 2);
        assert_eq!(r.summary.errors, 0);
        assert_eq!(r.summary.tables_checked, 2);
    }

    #[test]
    fn files_checked_skips_empty_paths() {
        let o = outcome(&["", "real.xlsx"], &[], &[]);
        let r = Report::from_outcome(&o, "v", "dir", "standard");
        assert_eq!(r.summary.files_checked, 1, "only real.xlsx counts");
    }

    #[test]
    fn empty_outcome_yields_all_zero_summary_and_no_passed() {
        let o = outcome(&[], &[], &[]);
        let r = Report::from_outcome(&o, "0.1.0", "dir", "standard");
        assert_eq!(r.summary.errors, 0);
        assert_eq!(r.summary.warnings, 0);
        assert_eq!(r.summary.files_checked, 0);
        assert_eq!(r.summary.tables_checked, 0);
        assert!(r.diagnostics.is_empty());
        assert!(r.passed_sheets.is_empty());
        assert_eq!(r.schema_version, REPORT_SCHEMA_VERSION);
        assert_eq!(r.tablec_version, "0.1.0");
        assert_eq!(r.input_dir, "dir");
        assert_eq!(r.parser, "standard");
    }

    #[test]
    fn no_files_outcome_counts_single_warning_no_files_no_tables() {
        // The pipeline's "no matching files" marker: one warning diagnostic
        // with a default (file-less) location and no tables.
        let o = outcome(&[], &[], &[diag(Severity::Warning, "no files", None, None)]);
        let r = Report::from_outcome(&o, "v", "empty-dir", "standard");
        assert_eq!(r.summary.warnings, 1);
        assert_eq!(r.summary.errors, 0);
        assert_eq!(r.summary.files_checked, 0);
        assert_eq!(r.summary.tables_checked, 0);
    }

    #[test]
    fn metadata_fields_are_taken_from_the_caller() {
        let o = outcome(&[], &["A"], &[]);
        let r = Report::from_outcome(&o, "9.9.9", "/data/tables", "my-plugin");
        assert_eq!(r.tablec_version, "9.9.9");
        assert_eq!(r.input_dir, "/data/tables");
        assert_eq!(r.parser, "my-plugin");
    }

    // -------------------------------------------------------------------
    // 1.1 Passed sheets
    // -------------------------------------------------------------------

    #[test]
    fn passed_sheets_exclude_tables_named_by_a_diagnostic() {
        // Sheet A carries a diagnostic; sheet B parsed clean.
        let o = outcome(
            &["f.xlsx"],
            &["A", "B"],
            &[diag(Severity::Error, "e", Some("f.xlsx"), Some("A"))],
        );
        let r = Report::from_outcome(&o, "v", "dir", "standard");
        assert_eq!(r.passed_sheets, vec!["B".to_string()]);
    }

    #[test]
    fn passed_sheets_keep_table_order_and_cover_all_clean_tables() {
        let o = outcome(&[], &["A", "B", "C"], &[]);
        let r = Report::from_outcome(&o, "v", "dir", "standard");
        assert_eq!(
            r.passed_sheets,
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
    }

    // -------------------------------------------------------------------
    // 1.2 RFC 3339 UTC formatter — known vectors
    // -------------------------------------------------------------------

    #[test]
    fn rfc3339_epoch_zero() {
        assert_eq!(format_rfc3339_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn rfc3339_end_of_first_day() {
        assert_eq!(format_rfc3339_utc(86_399), "1970-01-01T23:59:59Z");
    }

    #[test]
    fn rfc3339_non_leap_year_rollover() {
        // 1970 is not a leap year: day 58 is Feb 28, day 59 is Mar 1.
        assert_eq!(format_rfc3339_utc(58 * 86_400), "1970-02-28T00:00:00Z");
        assert_eq!(format_rfc3339_utc(59 * 86_400), "1970-03-01T00:00:00Z");
    }

    #[test]
    fn rfc3339_leap_day() {
        // 2000-02-29T00:00:00Z
        assert_eq!(format_rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn rfc3339_year_boundary() {
        assert_eq!(format_rfc3339_utc(1_704_067_200), "2024-01-01T00:00:00Z");
        assert_eq!(
            format_rfc3339_utc(1_704_067_200 - 1),
            "2023-12-31T23:59:59Z"
        );
    }

    #[test]
    fn rfc3339_century_rollover_not_a_leap_year() {
        // 2100 is divisible by 100 but not 400 → not a leap year.
        assert_eq!(format_rfc3339_utc(4_102_444_800), "2100-01-01T00:00:00Z");
        // Its Feb has 28 days: Feb 28 + 1 day = Mar 1.
        assert_eq!(
            format_rfc3339_utc(4_102_444_800 + 31 * 86_400 + 27 * 86_400),
            "2100-02-28T00:00:00Z"
        );
        assert_eq!(
            format_rfc3339_utc(4_102_444_800 + 31 * 86_400 + 28 * 86_400),
            "2100-03-01T00:00:00Z"
        );
    }

    #[test]
    fn rfc3339_intraday_components() {
        // 12:34:56 on the epoch day.
        assert_eq!(
            format_rfc3339_utc(12 * 3600 + 34 * 60 + 56),
            "1970-01-01T12:34:56Z"
        );
    }

    /// Structural RFC 3339 UTC check: `YYYY-MM-DDTHH:MM:SSZ`, exactly 20
    /// chars, digits at the expected positions — no fractional seconds.
    fn assert_rfc3339_shape(s: &str) {
        assert_eq!(s.len(), 20, "expected second-precision form, got {s:?}");
        let b = s.as_bytes();
        assert_eq!(b[4], b'-');
        assert_eq!(b[7], b'-');
        assert_eq!(b[10], b'T');
        assert_eq!(b[13], b':');
        assert_eq!(b[16], b':');
        assert_eq!(b[19], b'Z');
        for (i, c) in b.iter().enumerate() {
            if !matches!(i, 4 | 7 | 10 | 13 | 16 | 19) {
                assert!(c.is_ascii_digit(), "expected digit at {i} in {s:?}");
            }
        }
    }

    #[test]
    fn rfc3339_sub_second_truncation_shape() {
        // now_rfc3339_utc must never carry a fractional part: the builder
        // truncates SystemTime to whole seconds before formatting.
        let s = Report::from_outcome(&outcome(&[], &[], &[]), "v", "d", "p").generated_at;
        assert_rfc3339_shape(&s);
        assert!(!s.contains('.'), "no fractional seconds in {s:?}");
    }

    #[test]
    fn from_outcome_pins_generated_at_to_the_given_instant() {
        let o = outcome(&[], &[], &[]);
        let r = Report::from_outcome_at(&o, "v", "d", "p", 1_704_067_200);
        assert_eq!(r.generated_at, "2024-01-01T00:00:00Z");
    }
}
