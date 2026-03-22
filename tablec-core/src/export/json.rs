use std::error::Error;
use crate::core::project::project::Project;
use crate::export::Format;
use serde_json::json;

/// JSON export format
pub struct Json {
    pub pretty: bool,
    pub include_fields: bool,
}

impl Format for Json {
    fn export(&self, project: &Project, output: &str) -> Result<(), Box<dyn Error>> {
        let data = self.to_vec(project)?;
        if let Some(parent) = std::path::Path::new(output).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output, data)?;
        println!("Exported data to {}", output);
        Ok(())
    }

    fn to_vec(&self, project: &Project) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut tables_json = Vec::new();

        for (table_name, table) in &project.tables {
            let mut table_json = json!({
                "name": table_name,
                "data": table.data
            });

            if self.include_fields {
                table_json["fields"] = json!(table.fields);
            }

            tables_json.push(table_json);
        }

        let project_json = json!({
            "name": project.name,
            "meta": project.meta,
            "tables": tables_json
        });

        let data = if self.pretty {
            serde_json::to_string_pretty(&project_json)?
        } else {
            serde_json::to_string(&project_json)?
        };

        Ok(data.into_bytes())
    }
}

/// Legacy function for backward compatibility
pub fn to_string(tabs: &Vec<crate::core::table::table::Table>, include_fields: bool) -> Result<String, Box<dyn Error>> {
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
