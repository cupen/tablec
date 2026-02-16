use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use crate::core::table::table::Table;
use super::meta::Meta;

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
        let tables = crate::core::table::table::read_excel(path)?;
        Ok(Self::from_tables(name, tables))
    }
}