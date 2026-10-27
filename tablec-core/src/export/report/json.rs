//! JSON validation report renderer.
//!
//! The envelope is the `Serialize` shape of [`Report`], pretty-printed so CI
//! artifacts stay diffable. The top-level member set for
//! [`REPORT_SCHEMA_VERSION`] 1 is locked by the structure test below; any
//! change that breaks consumers of the envelope or the diagnostics array
//! shape must bump the version.

use super::Report;

/// Schema version of the JSON validation report envelope. Bump on any
/// change that breaks consumers of the envelope structure or the
/// diagnostics array shape (see the validation-report spec).
pub const REPORT_SCHEMA_VERSION: u32 = 1;

/// Top-level member set of the JSON envelope for schema version 1.
#[cfg(test)]
const ENVELOPE_MEMBERS_V1: [&str; 7] = [
    "schema_version",
    "tablec_version",
    "generated_at",
    "input_dir",
    "parser",
    "summary",
    "diagnostics",
];

/// Member set of the `summary` object for schema version 1.
#[cfg(test)]
const SUMMARY_MEMBERS_V1: [&str; 4] = ["errors", "warnings", "files_checked", "tables_checked"];

impl Report {
    /// Render the report as pretty-printed JSON (a diffable CI artifact).
    /// Serialization of this plain-data struct cannot fail.
    pub fn render_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Report serialization cannot fail")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::check::CheckOutcome;
    use crate::core::diagnostic::{Diagnostic, DiagnosticCode, Severity, SourceLocation};
    use std::path::PathBuf;

    fn diag(sev: Severity, code: DiagnosticCode, msg: &str, sheet: Option<&str>) -> Diagnostic {
        Diagnostic {
            severity: sev,
            code,
            message: msg.to_string(),
            location: SourceLocation {
                file: Some(PathBuf::from("tables/data.xlsx")),
                sheet: sheet.map(str::to_string),
                line: Some(6),
                column: Some(2),
            },
        }
    }

    fn sample_report() -> Report {
        let outcome = CheckOutcome {
            tables: vec![crate::core::table::table::Table {
                name: "Item".to_string(),
                schema: crate::core::schema::Schema::from_parts(vec![], vec![]),
                data: vec![],
            }],
            diagnostics: vec![
                diag(
                    Severity::Error,
                    DiagnosticCode::ValueOutOfRange,
                    "200 not in int8 range [-128, 127]",
                    Some("Item"),
                ),
                diag(
                    Severity::Warning,
                    DiagnosticCode::ConstraintDuplicate,
                    "@unique: duplicated value 1",
                    Some("Drop"),
                ),
            ],
            files: vec![std::path::PathBuf::from("tables.xlsx")],
        };
        Report::from_outcome_at(&outcome, "0.1.0", "tables", "standard", 0)
    }

    // -------------------------------------------------------------------
    // Structure lock: exact member set for schema_version 1
    // -------------------------------------------------------------------

    #[test]
    fn json_structure_locks_exact_top_level_member_set() {
        let v: serde_json::Value =
            serde_json::from_str(&sample_report().render_json()).expect("valid JSON");
        let obj = v.as_object().expect("top-level object");
        let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        assert_eq!(
            keys, ENVELOPE_MEMBERS_V1,
            "the JSON v1 envelope member set is a contract — bump REPORT_SCHEMA_VERSION to change it"
        );
        assert_eq!(v["schema_version"], 1);
    }

    #[test]
    fn json_summary_locks_exact_member_set() {
        let v: serde_json::Value =
            serde_json::from_str(&sample_report().render_json()).expect("valid JSON");
        let summary = v["summary"].as_object().expect("summary object");
        let keys: Vec<&str> = summary.keys().map(String::as_str).collect();
        assert_eq!(
            keys, SUMMARY_MEMBERS_V1,
            "the JSON v1 summary member set is a contract"
        );
    }

    #[test]
    fn json_is_pretty_printed() {
        let s = sample_report().render_json();
        assert!(s.contains('\n'), "pretty output is multi-line");
        assert!(s.starts_with("{\n  \"schema_version\""), "got: {s}");
    }

    #[test]
    fn json_envelope_carries_metadata() {
        let v: serde_json::Value =
            serde_json::from_str(&sample_report().render_json()).expect("valid JSON");
        assert_eq!(v["tablec_version"], "0.1.0");
        assert_eq!(v["input_dir"], "tables");
        assert_eq!(v["parser"], "standard");
        assert_eq!(v["generated_at"], "1970-01-01T00:00:00Z");
    }

    // -------------------------------------------------------------------
    // Summary counters match the builder
    // -------------------------------------------------------------------

    #[test]
    fn json_summary_counters_match_builder() {
        let r = sample_report();
        let v: serde_json::Value = serde_json::from_str(&r.render_json()).expect("valid JSON");
        assert_eq!(v["summary"]["errors"], r.summary.errors);
        assert_eq!(v["summary"]["warnings"], r.summary.warnings);
        assert_eq!(v["summary"]["files_checked"], r.summary.files_checked);
        assert_eq!(v["summary"]["tables_checked"], r.summary.tables_checked);
        assert_eq!(v["summary"]["errors"], 1);
        assert_eq!(v["summary"]["warnings"], 1);
        assert_eq!(v["summary"]["tables_checked"], 1);
    }

    // -------------------------------------------------------------------
    // Diagnostics passthrough in pipeline order + core serde equality
    // -------------------------------------------------------------------

    #[test]
    fn json_diagnostics_preserve_pipeline_order() {
        let v: serde_json::Value =
            serde_json::from_str(&sample_report().render_json()).expect("valid JSON");
        let diags = v["diagnostics"].as_array().expect("diagnostics array");
        assert_eq!(diags.len(), 2);
        assert_eq!(
            diags[0]["message"], "200 not in int8 range [-128, 127]",
            "first pipeline diagnostic must come first"
        );
        assert_eq!(
            diags[1]["message"], "@unique: duplicated value 1",
            "second pipeline diagnostic must come second"
        );
    }

    #[test]
    fn json_diagnostic_entry_equals_core_serde_output() {
        // The report's diagnostics entries must use the exact serialization
        // of the core diagnostic model — the same shape the webui
        // `/api/check` endpoint serves.
        let r = sample_report();
        let v: serde_json::Value = serde_json::from_str(&r.render_json()).expect("valid JSON");
        for (reported, core) in v["diagnostics"]
            .as_array()
            .expect("diagnostics array")
            .iter()
            .zip(&r.diagnostics)
        {
            assert_eq!(
                reported,
                &serde_json::to_value(core).expect("core diag serde")
            );
        }
    }

    #[test]
    fn json_clean_run_has_empty_diagnostics_array() {
        let outcome = CheckOutcome {
            tables: vec![],
            diagnostics: vec![],
            files: vec![std::path::PathBuf::from("a.xlsx")],
        };
        let r = Report::from_outcome_at(&outcome, "v", "d", "p", 0);
        let v: serde_json::Value = serde_json::from_str(&r.render_json()).expect("valid JSON");
        assert!(v["diagnostics"].as_array().expect("array").is_empty());
        assert_eq!(v["summary"]["errors"], 0);
        assert_eq!(v["summary"]["warnings"], 0);
        assert_eq!(v["summary"]["files_checked"], 1);
    }
}
