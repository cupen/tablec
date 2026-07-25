mod common;
use common::expect_diagnostic;
use tablec_core::core::diagnostic::*;

#[test]
fn helper_finds_code() {
    let errs = vec![
        Diagnostic::new(DiagnosticCode::SheetSkipped, "x", SourceLocation::default()),
        Diagnostic::new(
            DiagnosticCode::ValueParseError,
            "y",
            SourceLocation::default(),
        ),
    ];
    let d = expect_diagnostic(&errs, DiagnosticCode::ValueParseError);
    assert_eq!(d.message, "y");
}

#[test]
fn read_excel_propagates_diagnostics_on_bad_value() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/error_cases/bad_int_range.xlsx");
    let errs = tablec_core::core::table::table::read_excel(path.to_str().unwrap())
        .err()
        .expect("expected Err");
    // Aggregation: both out-of-range cells must surface.
    let n = errs
        .iter()
        .filter(|d| d.code == tablec_core::core::diagnostic::DiagnosticCode::ValueOutOfRange)
        .count();
    assert_eq!(n, 2, "expected 2 ValueOutOfRange diagnostics, got {}", n);
}

#[test]
fn read_excel_propagates_struct_field_mismatch() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/error_cases/bad_struct_field.xlsx");
    let errs = tablec_core::core::table::table::read_excel(path.to_str().unwrap())
        .err()
        .expect("expected Err");
    assert!(errs.iter().any(|d| d.code
        == tablec_core::core::diagnostic::DiagnosticCode::StructFieldMismatch
        || d.code == tablec_core::core::diagnostic::DiagnosticCode::StructFieldCountMismatch));
}
