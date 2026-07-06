#[test]
fn table_level_unique_duplicate_reports_diagnostic() {
    use tablec_core::core::diagnostic::DiagnosticCode;
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/error_cases/bad_unique_constraint.xlsx");
    let tables = tablec_core::core::table::table::read_excel(path.to_str().unwrap()).unwrap();
    let errs = tables[0].validate_constraints().unwrap_err();
    assert!(errs.iter().any(|d| d.code == DiagnosticCode::ConstraintDuplicate));
}