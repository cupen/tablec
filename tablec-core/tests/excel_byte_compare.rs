//! 字节级一致性测试：read_excel_with(StandardSchemaParser) 与 read_excel 输出 Table 字段级一致
use std::fs;
use std::path::PathBuf;
use tablec_core::core::diagnostic::DiagnosticCode;
use tablec_core::core::schema::{Schema, SchemaParseResult, SchemaParser, StandardSchemaParser};
use tablec_core::core::table::field::{Field, FieldType};
use tablec_core::core::table::table::read_excel;

fn list_test_xlsx() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testdata");
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("xlsx") {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn read_excel_with_standard_matches_read_excel_byte_level() {
    let xs = list_test_xlsx();
    assert!(
        !xs.is_empty(),
        "no fixture xlsx found in tests/fixtures/testdata/ — byte-level test would be a no-op"
    );
    for p in xs {
        let old =
            read_excel(p.to_str().unwrap()).unwrap_or_else(|e| panic!("{}: {:?}", p.display(), e));
        let new = tablec_core::core::table::table::read_excel_with(
            p.to_str().unwrap(),
            &StandardSchemaParser,
        )
        .unwrap_or_else(|e| panic!("{}: {:?}", p.display(), e));
        assert_eq!(old.len(), new.len(), "table count: {}", p.display());
        for (o, n) in old.iter().zip(new.iter()) {
            assert_eq!(o.name, n.name, "table name");
            assert_eq!(o.schema.fields, n.schema.fields, "fields for {}", o.name);
            assert_eq!(
                o.schema.constraints, n.schema.constraints,
                "constraints for {}",
                o.name
            );
            assert_eq!(o.data.len(), n.data.len(), "row count for {}", o.name);
            for (or, nr) in o.data.iter().zip(n.data.iter()) {
                assert_eq!(or.fields, nr.fields, "row fields for {}", o.name);
            }
        }
    }
}

struct DuplicateFieldParser;
impl SchemaParser for DuplicateFieldParser {
    fn name(&self) -> &str {
        "dup"
    }
    fn parse_schema(
        &self,
        _: &str,
        _: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<tablec_core::core::diagnostic::Diagnostic>> {
        let field = || Field {
            name: "id".to_string(),
            t: FieldType::Int32,
            desc: String::new(),
            constraint: None,
            tags: vec![],
        };
        Ok(SchemaParseResult::Schema(Schema {
            fields: vec![field(), field()],
            constraints: vec![],
            data_start_row: 5,
        }))
    }
}

struct OutOfBoundsParser;
impl SchemaParser for OutOfBoundsParser {
    fn name(&self) -> &str {
        "oob"
    }
    fn parse_schema(
        &self,
        _: &str,
        _: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<tablec_core::core::diagnostic::Diagnostic>> {
        Ok(SchemaParseResult::Schema(Schema {
            fields: vec![],
            constraints: vec![],
            data_start_row: 999,
        }))
    }
}

#[test]
fn duplicate_field_name_yields_schema_field_overlap() {
    let fpath = list_test_xlsx()
        .into_iter()
        .next()
        .expect("need at least one fixture xlsx");
    let err = tablec_core::core::table::table::read_excel_with(
        fpath.to_str().unwrap(),
        &DuplicateFieldParser,
    )
    .unwrap_err();
    assert!(
        err.iter()
            .any(|d| d.code == DiagnosticCode::SchemaFieldOverlap)
    );
}

#[test]
fn data_start_row_oob_yields_schema_data_start_oob() {
    let fpath = list_test_xlsx()
        .into_iter()
        .next()
        .expect("need at least one fixture xlsx");
    let err = tablec_core::core::table::table::read_excel_with(
        fpath.to_str().unwrap(),
        &OutOfBoundsParser,
    )
    .unwrap_err();
    assert!(
        err.iter()
            .any(|d| d.code == DiagnosticCode::SchemaDataStartOOB
                && d.message.contains("data_start_row=999"))
    );
}
