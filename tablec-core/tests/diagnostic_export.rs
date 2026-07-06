mod common;
use common::expect_diagnostic;
use tablec_core::core::diagnostic::*;

#[test]
fn helper_finds_code() {
    let errs = vec![
        Diagnostic::new(DiagnosticCode::SheetSkipped, "x", SourceLocation::default()),
        Diagnostic::new(DiagnosticCode::ValueParseError, "y", SourceLocation::default()),
    ];
    let d = expect_diagnostic(&errs, DiagnosticCode::ValueParseError);
    assert_eq!(d.message, "y");
}
