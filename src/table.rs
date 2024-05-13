
use calamine::{Reader, open_workbook_auto};
use serde::{Serialize};
use serde_json;
use std::error::Error;

mod fields;
use fields::Type;

mod parser;
use parser::parse_array;
use parser::parse_map;

#[derive(Debug, Serialize)]
struct Table {
    name: String,
    fields: Vec<String>,
    types: Vec<Type>,
    comments: Vec<String>,
    data: Vec<Vec<String>>,
}



fn read_excel(fpath: &str) -> Result<Vec<Table>, Box<dyn Error>> {
    let mut workbook = open_workbook_auto(fpath)?;
    let mut tables = vec![];

    for sheet_name in workbook.sheet_names().to_owned() {
        if sheet_name.starts_with('#') {
            continue;
        }

        let sheet = workbook.worksheet_range(&sheet_name).unwrap();

        let mut fields = vec![];
        let mut types = vec![];
        let mut comments = vec![];
        let mut data = vec![];
        for (i, row) in sheet.rows().enumerate() {
            match i {
                0 => fields = row.iter().map(|cell| cell.to_string()).collect(),
                1 => types = row.iter().map(|cell| Type::from_str(&cell.to_string()).unwrap()).collect(),
                2 => comments = row.iter().map(|cell| cell.to_string()).collect(),
                _ => {
                    let mut row_data = vec![];
                    for (j, cell) in row.iter().enumerate() {
                        let cell_value = cell.to_string();
                        match &types[j] {
                            Type::Int => {
                                if cell_value.parse::<i32>().is_err() {
                                    return Err(format!("Error at row {}, column {}: expected int, found '{}'", i + 1, j + 1, cell_value).into());
                                }
                            }
                            Type::String => {}
                            Type::Float => {
                                if cell_value.parse::<f64>().is_err() {
                                    return Err(format!("Error at row {}, column {}: expected float, found '{}'", i + 1, j + 1, cell_value).into());
                                }
                            }
                            Type::ArrayInt => {
                                if parse_array::<i32>(&cell_value).is_err() {
                                    return Err(format!("Error at row {}, column {}: expected array<int>, found '{}'", i + 1, j + 1, cell_value).into());
                                }
                            }
                            Type::ArrayString => {
                                if parse_array::<String>(&cell_value).is_err() {
                                    return Err(format!("Error at row {}, column {}: expected array<string>, found '{}'", i + 1, j + 1, cell_value).into());
                                }
                            }
                        }
                        row_data.push(cell_value);
                    }
                    data.push(row_data);
                }
            }

            tables.push(Table{
                name: sheet_name.to_owned(),
                fields: fields,
                types: types,
                comments: comments,
                data: data,
            });
        }
    }
    Ok(tables)
}


fn export_to_json(tab: &Table, output: &str) -> Result<(), Box<dyn Error>> {
    let json_data = serde_json::to_string_pretty(tab)?;
    std::fs::write(output, json_data)?;
    println!("Exported data to {}", output);
    Ok(())
}

// Function to parse array types
fn parse_array<T: std::str::FromStr>(text: &str) -> Result<Vec<T>, Box<dyn Error>> {
    // 
    let trimmed = text.trim_matches(|c| c == '[' || c == ']');
    let items: Vec<&str> = trimmed.split(", ").collect();
    let mut result = vec![];
    for item in items {
        let parsed_value = item.trim().parse::<T>()?;
        result.push(parsed_value);
    }
    Ok(result)
}

