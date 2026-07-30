use std::collections::HashMap;
use std::sync::Arc;

use super::table::constraint::Constraint;
use super::table::field::Field;
use serde::{Deserialize, Serialize};

pub mod dynamic;
#[cfg(any(test, doc))]
pub mod example;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schema {
    pub fields: Vec<Field>,
    pub constraints: Vec<Constraint>,
    pub data_start_row: usize,
}

impl Schema {
    /// 兼容旧调用方：fields/constraints 直接给，自动设 data_start_row = 5
    pub fn from_parts(fields: Vec<Field>, constraints: Vec<Constraint>) -> Schema {
        Schema {
            fields,
            constraints,
            data_start_row: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::table::field::{Field, FieldType};

    fn dummy_field(name: &str) -> Field {
        Field {
            name: name.to_string(),
            t: FieldType::Int32,
            desc: String::new(),
            constraint: None,
            tags: vec![],
        }
    }

    #[test]
    fn schema_from_parts_defaults_data_start_row_to_5() {
        let s = Schema::from_parts(vec![dummy_field("id")], vec![]);
        assert_eq!(s.data_start_row, 5);
    }

    #[test]
    fn schema_struct_literal_sets_data_start_row_explicitly() {
        let s = Schema {
            fields: vec![dummy_field("id")],
            constraints: vec![],
            data_start_row: 8,
        };
        assert_eq!(s.data_start_row, 8);
    }

    #[test]
    fn schema_clone_equals_original() {
        let s = Schema::from_parts(vec![dummy_field("id")], vec![]);
        assert_eq!(s.clone(), s);
    }
}

pub trait SchemaParser: Send + Sync {
    fn name(&self) -> &str;
    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<crate::core::diagnostic::Diagnostic>>;
}

pub enum SchemaParseResult {
    Schema(Schema),
    Skip,
}

#[cfg(test)]
mod trait_tests {
    use super::*;
    use crate::core::diagnostic::Diagnostic;

    struct MockParser;

    impl SchemaParser for MockParser {
        fn name(&self) -> &str {
            "mock"
        }
        fn parse_schema(
            &self,
            _sheet_name: &str,
            _sheet: &[Vec<String>],
        ) -> Result<SchemaParseResult, Vec<Diagnostic>> {
            Ok(SchemaParseResult::Skip)
        }
    }

    #[test]
    fn mock_parser_name() {
        assert_eq!(MockParser.name(), "mock");
    }

    #[test]
    fn mock_parser_returns_skip() {
        let p = MockParser;
        let r = p.parse_schema("foo", &[]).unwrap();
        assert!(matches!(r, SchemaParseResult::Skip));
    }

    #[test]
    fn trait_object_dispatch_works() {
        let p: Box<dyn SchemaParser> = Box::new(MockParser);
        assert_eq!(p.name(), "mock");
    }
}

#[cfg(test)]
mod standard_parser_tests {
    use super::*;
    use crate::core::diagnostic::Diagnostic;
    use crate::core::table::field::{Field, FieldType};

    fn sheet_with_rows(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter()
            .map(|r| r.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn parses_5_row_layout() {
        let p = StandardSchemaParser;
        let sheet = sheet_with_rows(&[
            &["id", "name"],
            &["int", "string"],
            &["ID", "Name"],
            &["", ""],
            &["", ""],
            &["1", "alice"],
            &["2", "bob"],
        ]);
        let r = p.parse_schema("T", &sheet).unwrap();
        match r {
            SchemaParseResult::Schema(s) => {
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name, "id");
                assert_eq!(s.fields[0].t, FieldType::Int32);
                assert_eq!(s.fields[1].name, "name");
                assert_eq!(s.fields[1].t, FieldType::String);
                assert_eq!(s.data_start_row, 5);
            }
            _ => panic!("expected Schema"),
        }
    }

    #[test]
    fn first_column_hash_returns_skip() {
        let p = StandardSchemaParser;
        let sheet = sheet_with_rows(&[&["#comment"], &["id"], &["int"]]);
        let r = p.parse_schema("T", &sheet).unwrap();
        assert!(matches!(r, SchemaParseResult::Skip));
    }

    #[test]
    fn empty_sheet_returns_error() {
        let p = StandardSchemaParser;
        let r = p.parse_schema("T", &[]);
        assert!(r.is_err());
    }

    #[test]
    fn field_name_with_tags_is_split() {
        let p = StandardSchemaParser;
        let sheet = sheet_with_rows(&[&["id[client,key]"], &["int"], &[""], &[""], &[""], &["1"]]);
        let r = p.parse_schema("T", &sheet).unwrap();
        match r {
            SchemaParseResult::Schema(s) => {
                assert_eq!(s.fields[0].name, "id");
                assert_eq!(s.fields[0].tags, vec!["client", "key"]);
            }
            _ => panic!("expected Schema"),
        }
    }

    #[test]
    fn unparseable_type_falls_back_to_string() {
        let p = StandardSchemaParser;
        let sheet = sheet_with_rows(&[&["x"], &["not_a_type"], &[""], &[""], &[""], &["v"]]);
        let r = p.parse_schema("T", &sheet).unwrap();
        match r {
            SchemaParseResult::Schema(s) => {
                assert_eq!(s.fields[0].t, FieldType::String);
            }
            _ => panic!("expected Schema"),
        }
    }

    #[test]
    fn missing_columns_padded_with_empty() {
        let p = StandardSchemaParser;
        let sheet = sheet_with_rows(&[
            &["a", "b"],
            &["int"], // 缺 b 列
            &[""],
            &[""],
            &[""],
        ]);
        let r = p.parse_schema("T", &sheet).unwrap();
        match r {
            SchemaParseResult::Schema(s) => {
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[1].name, "b");
                assert_eq!(s.fields[1].t, FieldType::String); // 缺类型 fallback
            }
            _ => panic!("expected Schema"),
        }
    }

    #[test]
    fn name_returns_standard() {
        assert_eq!(StandardSchemaParser.name(), "standard");
    }
}

pub struct StandardSchemaParser;

impl SchemaParser for StandardSchemaParser {
    fn name(&self) -> &str {
        "standard"
    }

    fn parse_schema(
        &self,
        sheet_name: &str,
        sheet: &[Vec<String>],
    ) -> Result<SchemaParseResult, Vec<crate::core::diagnostic::Diagnostic>> {
        use crate::core::diagnostic::{Diagnostic, DiagnosticCode, SourceLocation};
        use crate::core::table::constraint::Constraint;
        use crate::core::table::field::{Field, FieldType};
        use std::str::FromStr;

        if sheet.is_empty() {
            return Err(vec![Diagnostic::new(
                DiagnosticCode::HeaderParserError,
                "sheet is empty".to_string(),
                SourceLocation::default(),
            )]);
        }

        // 首列以 # 开头 → skip
        if sheet[0]
            .first()
            .map(|s| s.starts_with('#'))
            .unwrap_or(false)
        {
            return Ok(SchemaParseResult::Skip);
        }

        let get_row = |idx: usize| -> Vec<String> { sheet.get(idx).cloned().unwrap_or_default() };

        let field_names = get_row(0);
        let field_types_str = get_row(1);
        let field_comments = get_row(2);
        let constraint_str = get_row(3);
        let row5 = get_row(4);

        // 表级约束
        let mut table_constraints = Vec::new();
        for (col_idx, raw) in row5.iter().enumerate() {
            let cell = raw.trim();
            if cell.is_empty() {
                continue;
            }
            if !cell.starts_with('@') {
                return Err(vec![Diagnostic::new(
                    DiagnosticCode::TableConstraintParseError,
                    format!(
                        "row 5 cell {} must start with @, got '{}'",
                        col_idx + 1,
                        cell
                    ),
                    SourceLocation::default(),
                )]);
            }
            let loc = SourceLocation::default();
            match Constraint::from_str_with_loc(cell, loc) {
                Ok(c) => table_constraints.push(c),
                Err(d) => return Err(vec![d]),
            }
        }

        let mut fields = Vec::new();
        for i in 0..field_names.len() {
            let name = field_names.get(i).cloned().unwrap_or_default();
            if name.is_empty() || name.starts_with('#') {
                continue;
            }

            let raw_constraint = constraint_str.get(i).cloned().unwrap_or_default();

            fields.push(Field {
                name: name.split('[').next().unwrap_or(&name).trim().to_string(),
                t: FieldType::from_str(field_types_str.get(i).map(|s| s.as_str()).unwrap_or(""))
                    .unwrap_or(FieldType::String),
                desc: field_comments.get(i).cloned().unwrap_or_default(),
                constraint: Constraint::from_str(&raw_constraint).ok(),
                tags: {
                    let mut tags = Vec::new();
                    if let Some(start) = name.find('[') {
                        if let Some(end) = name.find(']') {
                            if end > start {
                                let tag_str = &name[start + 1..end];
                                tags.extend(tag_str.split(',').map(|s| s.trim().to_string()));
                            }
                        }
                    }
                    tags
                },
            });
        }

        Ok(SchemaParseResult::Schema(Schema {
            fields,
            constraints: table_constraints,
            // If the sheet has fewer than 5 rows, treat all rows as header
            // (i.e. 0 data rows) instead of failing OOB. This preserves the
            // legacy `read_excel` behavior of producing an empty-data Table
            // for short/malformed inputs. Custom parsers still go through
            // the strict OOB check in `read_excel_with`.
            data_start_row: std::cmp::min(5, sheet.len()),
        }))
    }
}

pub struct SchemaParserRegistry {
    parsers: HashMap<String, Arc<dyn SchemaParser>>,
}

impl SchemaParserRegistry {
    pub fn with_standard() -> Self {
        let mut r = Self {
            parsers: HashMap::new(),
        };
        r.register(StandardSchemaParser);
        r
    }

    /// Build a registry with `standard` + plugins loaded from `plugin_paths`.
    ///
    /// # Safety
    ///
    /// The caller must ensure `plugin_paths` points to trusted, ABI-compatible
    /// tablec plugins. Loading a `.so` runs arbitrary initializer code; this
    /// API does not validate plugin provenance.
    pub fn with_standard_and_plugins(
        plugin_paths: &[std::path::PathBuf],
    ) -> Result<Self, crate::core::schema::dynamic::DynamicPluginError> {
        let mut reg = Self::with_standard();
        for path in plugin_paths {
            // SAFETY: caller-provided plugin paths are trusted (see doc above).
            let plugin = unsafe { crate::core::schema::dynamic::DynamicPlugin::load(path) }?;
            let name = plugin.name().to_string();
            if reg.parsers.contains_key(&name) {
                return Err(crate::core::schema::dynamic::DynamicPluginError::DuplicateName(name));
            }
            reg.register_arc(plugin);
        }
        Ok(reg)
    }

    pub fn register_arc(&mut self, parser: Arc<dyn SchemaParser>) {
        let name = parser.name().to_string();
        if self.parsers.contains_key(&name) {
            panic!("parser '{}' already registered", name);
        }
        self.parsers.insert(name, parser);
    }

    pub fn register<P: SchemaParser + 'static>(&mut self, parser: P) {
        let name = parser.name().to_string();
        if self.parsers.contains_key(&name) {
            panic!("parser '{}' already registered", name);
        }
        self.parsers.insert(name, Arc::new(parser));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn SchemaParser>> {
        self.parsers.get(name).cloned()
    }

    pub fn parser_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.parsers.keys().cloned().collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn with_standard_contains_standard() {
        let reg = SchemaParserRegistry::with_standard();
        assert!(reg.get("standard").is_some());
        assert!(reg.parser_names().contains(&"standard".to_string()));
    }

    #[test]
    #[should_panic(expected = "parser 'standard' already registered")]
    fn register_same_name_panics() {
        let mut reg = SchemaParserRegistry::with_standard();
        reg.register(StandardSchemaParser);
    }

    #[test]
    #[should_panic(expected = "parser 'standard' already registered")]
    fn register_arc_panics_on_duplicate_name() {
        let mut reg = SchemaParserRegistry::with_standard();
        let arc: std::sync::Arc<dyn SchemaParser> = std::sync::Arc::new(StandardSchemaParser);
        reg.register_arc(arc);
    }
}

#[cfg(test)]
mod with_plugins_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_paths_yields_standard_only() {
        let reg = SchemaParserRegistry::with_standard_and_plugins(&[]).unwrap();
        assert!(reg.get("standard").is_some());
        assert_eq!(reg.parser_names(), vec!["standard".to_string()]);
    }

    #[test]
    fn nonexistent_plugin_path_yields_error() {
        let paths = vec![PathBuf::from("/tmp/tablec_no_such_plugin_xyz.so")];
        let r = SchemaParserRegistry::with_standard_and_plugins(&paths);
        assert!(r.is_err());
    }
}
