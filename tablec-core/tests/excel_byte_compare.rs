//! 字节级一致性测试：read_excel_with(StandardSchemaParser) 与 read_excel 输出 Table 字段级一致
use std::fs;
use std::path::PathBuf;
use tablec_core::core::schema::StandardSchemaParser;
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
