use rust_xlsxwriter::*;
fn main() {
    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet();
    sheet.write_string(0, 0, "data").unwrap(); // row 1: name
    sheet
        .write_string(1, 0, "struct{a:int32,b:string}")
        .unwrap(); // row 2: type
    sheet.write_string(2, 0, "a struct").unwrap(); // row 3: comment
    sheet.write_string(3, 0, "").unwrap(); // row 4: empty (constraints)
    sheet.write_string(4, 0, "").unwrap(); // row 5: empty
    // Row 6 data — struct missing field `b` should yield StructFieldCountMismatch
    sheet.write_string(5, 0, "{a: 1}").unwrap();
    wb.save("bad_struct_field.xlsx").unwrap();
}
