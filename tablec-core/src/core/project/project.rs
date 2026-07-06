use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use crate::core::table::table::Table;
use super::meta::Meta;
use super::super::config::Config;
use crate::export::Format;

fn read_excel_with_box(path: &str) -> Result<Vec<Table>, Box<dyn std::error::Error>> {
    crate::core::table::table::read_excel(path).map_err(|errs| {
        let msg = errs.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
        Box::<dyn std::error::Error>::from(msg)
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub meta: Meta,
    pub tables: IndexMap<String, Table>,
}

impl Project {
    pub fn from_tables(name: String, tables: Vec<Table>) -> Self {
        let mut table_map = IndexMap::new();
        for table in tables {
            table_map.insert(table.name.clone(), table);
        }

        Project {
            name,
            meta: Meta::default(),
            tables: table_map,
        }
    }

    pub fn from_excel(name: String, path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let tables = read_excel_with_box(path)?;
        Ok(Self::from_tables(name, tables))
    }

    pub fn from_config(config: &Config, input_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let tables = read_excel_with_box(input_path)?;
        let name = config.project.name.clone();
        Ok(Self::from_tables(name, tables))
    }
    
    pub fn calculate_hash(&mut self) {
        let mut hasher = DefaultHasher::new();
        self.name.hash(&mut hasher);
        
        for (table_name, table) in &self.tables {
            table_name.hash(&mut hasher);
            for row in &table.data {
                // Hash row data
                let row_str = format!("{:?}", row.fields);
                row_str.hash(&mut hasher);
            }
        }
        
        self.meta.hash = hasher.finish() as i64;
    }
    
    pub fn validate_all(&self) -> Result<(), Vec<String>> {
        let mut all_errors = Vec::new();

        for (table_name, table) in &self.tables {
            match table.validate_constraints() {
                Ok(_) => {},
                Err(errors) => {
                    for error in errors {
                        all_errors.push(format!("Table '{}': {}", table_name, error));
                    }
                }
            }
        }

        if all_errors.is_empty() {
            Ok(())
        } else {
            Err(all_errors)
        }
    }

    pub fn export<F: Format>(&self, format: &F, output: &str) -> Result<(), Box<dyn Error>> {
        format.export(self, output)
    }
}