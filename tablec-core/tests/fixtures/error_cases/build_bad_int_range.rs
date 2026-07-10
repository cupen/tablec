use rust_xlsxwriter::*;
fn main() {
    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet();
    sheet.write_string(0, 0, "id").unwrap();     // row 1: name
    sheet.write_string(1, 0, "int8").unwrap();    // row 2: type
    sheet.write_string(2, 0, "id").unwrap();      // row 3: comment
    sheet.write_string(3, 0, "").unwrap();        // row 4: empty
    sheet.write_string(4, 0, "").unwrap();        // row 5: empty
    // Two out-of-range cells — verifies aggregation (must collect both).
    sheet.write_number(5, 0, 200).unwrap();       // row 6 data — out of int8 range
    sheet.write_number(6, 0, 300).unwrap();       // row 7 data — out of int8 range
    wb.save("bad_int_range.xlsx").unwrap();
}