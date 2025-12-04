use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use crate::core::table::table::Table;
use super::meta::Meta;

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub meta: Meta,
    pub tables: IndexMap<String, Table>,
}