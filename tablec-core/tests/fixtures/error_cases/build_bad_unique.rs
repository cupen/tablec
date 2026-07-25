use rust_xlsxwriter::*;
fn main() {
    let mut wb = Workbook::new();
    let sh = wb.add_worksheet();
    sh.write_string(0, 0, "id").unwrap();
    sh.write_string(0, 1, "name").unwrap();
    sh.write_string(1, 0, "int").unwrap();
    sh.write_string(1, 1, "string").unwrap();
    sh.write_string(2, 0, "").unwrap();
    sh.write_string(2, 1, "").unwrap();
    sh.write_string(3, 0, "@seq").unwrap();
    sh.write_string(3, 1, "").unwrap();
    sh.write_string(4, 0, "@unique(id, name)").unwrap();
    sh.write_number(5, 0, 1).unwrap();
    sh.write_string(5, 1, "alice").unwrap();
    sh.write_number(6, 0, 1).unwrap();
    sh.write_string(6, 1, "alice").unwrap(); // duplicate
    sh.write_number(7, 0, 2).unwrap();
    sh.write_string(7, 1, "bob").unwrap();
    wb.save("tests/fixtures/error_cases/bad_unique_constraint.xlsx")
        .unwrap();
}
