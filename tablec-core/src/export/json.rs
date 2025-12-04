use std::error::Error;
use crate::core::table::table::Table;
use serde_json::json;

pub fn to_string(tabs: &Vec<Table>, include_fields: bool) -> Result<String, Box<dyn Error>> {
    let mut output_tables = Vec::new();

    for table in tabs {
        let mut table_json = json!({ "name": table.name, "data": table.data });

        if include_fields {
            table_json["fields"] = json!(table.fields);
        }
        output_tables.push(table_json);
    }

    let json_data = serde_json::to_string_pretty(&output_tables)?;
    Ok(json_data)
}