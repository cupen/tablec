// 一键生成 fixture xlsx：`cargo run --example build_fixture_xlsx -p tablec-core`
// 产物在 tablec-core/tests/fixtures/cdylib_parser/fixtures/test.xlsx
use rust_xlsxwriter::*;
use std::path::PathBuf;

fn main() {
    let out_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cdylib_parser/fixtures");
    std::fs::create_dir_all(&out_dir).expect("create fixtures dir");
    let out_path = out_dir.join("test.xlsx");

    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet().set_name("items").unwrap();

    // 标准 5 行布局
    sheet.write_string(0, 0, "id").unwrap(); // row 1: name
    sheet.write_string(0, 1, "name").unwrap();
    sheet.write_string(1, 0, "int").unwrap(); // row 2: type
    sheet.write_string(1, 1, "string").unwrap();
    sheet.write_string(2, 0, "ID").unwrap(); // row 3: comment
    sheet.write_string(2, 1, "Name").unwrap();
    sheet.write_string(3, 0, "").unwrap(); // row 4: field constraint
    sheet.write_string(3, 1, "").unwrap();
    sheet.write_string(4, 0, "").unwrap(); // row 5: table constraint
    sheet.write_string(4, 1, "").unwrap();

    // data
    sheet.write_number(5, 0, 1).unwrap();
    sheet.write_string(5, 1, "alice").unwrap();
    sheet.write_number(6, 0, 2).unwrap();
    sheet.write_string(6, 1, "bob").unwrap();

    wb.save(&out_path).expect("save fixture xlsx");
    println!("wrote {}", out_path.display());
}
