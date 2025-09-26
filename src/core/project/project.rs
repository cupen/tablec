use indexmap::IndexMap;

pub struct Project {
    name: String,
    meta: Meta,
    tables: IndexMap<String, Table>,
}