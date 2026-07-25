use criterion::{Criterion, criterion_group, criterion_main};
use std::path::PathBuf;
use tablec_core::core::table::table::read_excel;

fn bench_parse(c: &mut Criterion) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures/basic_test.xlsx");

    if path.exists() {
        c.bench_function("parse_basic_excel", |b| {
            b.iter(|| {
                read_excel(&path.to_string_lossy().to_string()).unwrap();
            });
        });
    }
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
