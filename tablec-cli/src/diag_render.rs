use std::io::{self, Write};
use tablec_core::core::diagnostic::{Diagnostic, Severity};

pub(crate) fn render_diags<W: Write>(diags: &[Diagnostic], out: &mut W) -> io::Result<()> {
    for d in diags {
        // severity prefix + Diagnostic::Display + file suffix
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(out, "{}\t{}", sev, d)?;
        if let Some(file) = &d.location.file {
            write!(out, "\t{}", file.display())?;
        }
        writeln!(out)?;
    }
    Ok(())
}

// Currently test-only; wire into `tablec check` exit codes when CLI commands
// start propagating diagnostics to the process exit status.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn diag_exit_code(diags: &[Diagnostic]) -> i32 {
    if diags.iter().any(|d| matches!(d.severity, Severity::Error)) {
        1
    } else {
        0
    }
}

pub(crate) fn diag_summary(diags: &[Diagnostic]) -> String {
    let errors = diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .count();
    let warnings = diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Warning))
        .count();
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!(
            "{} {}",
            errors,
            if errors == 1 { "error" } else { "errors" }
        ));
    }
    if warnings > 0 {
        parts.push(format!(
            "{} {}",
            warnings,
            if warnings == 1 { "warning" } else { "warnings" }
        ));
    }
    if parts.is_empty() {
        "no issues".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use tablec_core::core::diagnostic::{DiagnosticCode, SourceLocation};

    fn diag_with(sev: Severity, msg: &str) -> Diagnostic {
        Diagnostic {
            severity: sev,
            code: DiagnosticCode::Other,
            message: msg.to_string(),
            location: SourceLocation::default(),
        }
    }

    #[test]
    fn render_diags_writes_one_line_per_diag() {
        let diags = vec![
            diag_with(Severity::Error, "a"),
            diag_with(Severity::Warning, "b"),
        ];
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&diags, &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        // Each diag takes exactly one line.
        assert_eq!(s.lines().count(), 2, "got: {:?}", s);
        // Both lines reference the diag message.
        assert!(s.contains("a"));
        assert!(s.contains("b"));
    }

    #[test]
    fn render_diags_includes_file() {
        let d = Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::TypeParseError,
            message: "bad".into(),
            location: SourceLocation {
                file: Some(std::path::PathBuf::from("/abs/x.xlsx")),
                sheet: Some("S".into()),
                line: Some(2),
                column: Some(5),
            },
        };
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&[d], &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(s.contains("/abs/x.xlsx"), "expected file path in {:?}", s);
        assert!(s.contains("S"), "expected sheet in {:?}", s);
        assert!(s.contains("2:5"), "expected line:col in {:?}", s);
    }

    #[test]
    fn render_diags_skips_missing_file_gracefully() {
        let d = diag_with(Severity::Error, "no loc");
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&[d], &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        // No panic, message present.
        assert!(s.contains("no loc"));
    }

    #[test]
    fn diag_exit_code_first_error_returns_1() {
        let diags = vec![
            diag_with(Severity::Error, "e"),
            diag_with(Severity::Warning, "w"),
        ];
        assert_eq!(diag_exit_code(&diags), 1);
    }

    #[test]
    fn diag_exit_code_only_warnings_returns_0() {
        let diags = vec![diag_with(Severity::Warning, "w")];
        assert_eq!(diag_exit_code(&diags), 0);
    }

    #[test]
    fn diag_exit_code_empty_returns_0() {
        assert_eq!(diag_exit_code(&[]), 0);
    }

    #[test]
    fn diag_summary_counts_severity() {
        let diags = vec![
            diag_with(Severity::Error, "e1"),
            diag_with(Severity::Error, "e2"),
            diag_with(Severity::Warning, "w1"),
        ];
        assert_eq!(diag_summary(&diags), "2 errors, 1 warning");
    }

    #[test]
    fn render_diags_severity_prefix_distinguishes_error_from_warning() {
        // The severity word ("error" or "warning") must appear as the
        // first token of each rendered line, separated by a tab from the
        // Diagnostic::Display body.
        let diags = vec![
            diag_with(Severity::Error, "boom"),
            diag_with(Severity::Warning, "careful"),
        ];
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&diags, &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2, "got: {:?}", s);
        assert!(
            lines[0].starts_with("error\t"),
            "expected error prefix on line 0, got {:?}",
            lines[0]
        );
        assert!(
            lines[1].starts_with("warning\t"),
            "expected warning prefix on line 1, got {:?}",
            lines[1]
        );
        // And neither severity word appears on the wrong line.
        assert!(!lines[0].starts_with("warning\t"));
        assert!(!lines[1].starts_with("error\t"));
    }

    #[test]
    fn render_diags_includes_line_col_block_without_file() {
        // Display for Diagnostic renders " <line>:<col>" when both are set,
        // independently of whether `file` is set. Existing
        // `render_diags_includes_file` exercises this with file+line:col
        // together; this test pins the line:col block when file is None
        // so a Display regression is caught even in the no-file path.
        let d = Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::Other,
            message: "boom".into(),
            location: SourceLocation {
                file: None,
                sheet: None,
                line: Some(7),
                column: Some(4),
            },
        };
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&[d], &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(s.contains("7:4"), "expected line:col block in {:?}", s);
        assert!(s.contains("boom"), "expected message in {:?}", s);
    }

    #[test]
    fn render_diags_includes_code_in_display() {
        // Diagnostic::Display renders the code via Debug (the variant
        // name). This locks that contract: a Diagnostic with code
        // `ValueParseError` produces a line containing that token.
        let d = Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode::ValueParseError,
            message: "bad token".into(),
            location: SourceLocation::default(),
        };
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&[d], &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(
            s.contains("ValueParseError"),
            "expected code in rendered output, got {:?}",
            s
        );
    }

    #[test]
    fn render_diags_empty_input_writes_nothing() {
        // No diagnostics -> no output (not even a newline). Catches a
        // future regression where someone accidentally adds a stray
        // writeln! before the loop body.
        let diags: Vec<Diagnostic> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&diags, &mut buf).unwrap();
        assert!(buf.is_empty(), "expected empty output, got {:?}", buf);
    }

    #[test]
    fn render_diags_no_trailing_tab_when_file_missing() {
        // When `file` is None, the line must end with the message (then
        // newline), with no stray tab before the newline.
        let d = diag_with(Severity::Error, "no loc");
        let mut buf: Vec<u8> = Vec::new();
        render_diags(&[d], &mut buf).unwrap();
        let s = std::str::from_utf8(&buf).unwrap();
        assert!(
            !s.ends_with("\t\n"),
            "expected no trailing tab before newline, got {:?}",
            s
        );
        // And the message is the last non-newline token.
        assert!(s.ends_with("no loc\n"), "got: {:?}", s);
    }

    #[test]
    fn diag_summary_no_issues_when_empty() {
        assert_eq!(diag_summary(&[]), "no issues");
    }

    #[test]
    fn diag_summary_singular_labels() {
        // `1 error` / `1 warning` use the singular form (no trailing s).
        assert_eq!(diag_summary(&[diag_with(Severity::Error, "e")]), "1 error");
        assert_eq!(
            diag_summary(&[diag_with(Severity::Warning, "w")]),
            "1 warning"
        );
    }

    #[test]
    fn diag_summary_errors_only_omits_warning_part() {
        let diags = vec![
            diag_with(Severity::Error, "e1"),
            diag_with(Severity::Error, "e2"),
        ];
        assert_eq!(diag_summary(&diags), "2 errors");
        assert!(
            !diag_summary(&diags).contains("warning"),
            "expected no warning part when there are no warnings"
        );
    }

    #[test]
    fn diag_summary_warnings_only_omits_error_part() {
        let diags = vec![
            diag_with(Severity::Warning, "w1"),
            diag_with(Severity::Warning, "w2"),
            diag_with(Severity::Warning, "w3"),
        ];
        assert_eq!(diag_summary(&diags), "3 warnings");
        assert!(
            !diag_summary(&diags).contains("error"),
            "expected no error part when there are no errors"
        );
    }

    #[test]
    fn diag_exit_code_error_after_warning_still_returns_1() {
        // `diag_exit_code` uses `any(...)`, so an error at any position
        // should return 1 — not just the first. This test reverses the
        // order from `first_error_returns_1` to lock the semantics.
        let diags = vec![
            diag_with(Severity::Warning, "w"),
            diag_with(Severity::Error, "e"),
        ];
        assert_eq!(diag_exit_code(&diags), 1);
    }

    // silence unused-import warning when `Hash` and `DefaultHasher` aren't used
    #[allow(dead_code)]
    fn _silence_hash() -> u64 {
        let mut h = DefaultHasher::new();
        "x".hash(&mut h);
        h.finish()
    }
}
