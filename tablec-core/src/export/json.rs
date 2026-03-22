use std::error::Error;
use crate::core::table::table::Table;
use crate::core::project::project::Project;
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

pub fn project_to_string(project: &Project, include_fields: bool) -> Result<String, Box<dyn Error>> {
    let mut tables_json = Vec::new();

    for (table_name, table) in &project.tables {
        let mut table_json = json!({
            "name": table_name,
            "data": table.data
        });

        if include_fields {
            table_json["fields"] = json!(table.fields);
        }

        tables_json.push(table_json);
    }

    let project_json = json!({
        "name": project.name,
        "meta": project.meta,
        "tables": tables_json
    });

    Ok(serde_json::to_string_pretty(&project_json)?)
}