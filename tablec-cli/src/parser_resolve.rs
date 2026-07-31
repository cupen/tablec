use std::path::PathBuf;
use std::sync::Arc;

use tablec_core::core::config::Config;
use tablec_core::core::schema::{SchemaParser, SchemaParserRegistry};

/// Resolve which `SchemaParser` to use given CLI flag, parsed config,
/// or default fallback ("standard").
///
/// Precedence: `cli_parser` > `config.parser.name` > `"standard"`.
///
/// Panics if a plugin cannot be loaded or the resolved name is not registered.
pub fn resolve_parser(
    config: &Config,
    cli_parser: Option<&str>,
    plugin_paths: &[PathBuf],
) -> Arc<dyn SchemaParser> {
    let name = cli_parser
        .map(|s| s.to_string())
        .or_else(|| config.parser.as_ref().map(|p| p.name.clone()))
        .unwrap_or_else(|| "standard".to_string());
    let reg = SchemaParserRegistry::with_standard_and_plugins(plugin_paths)
        .unwrap_or_else(|e| panic!("failed to load schema parser plugin: {}", e));
    reg.get(&name)
        .unwrap_or_else(|| panic!("parser '{}' not registered", name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablec_core::core::config::ParserConfig;

    #[test]
    fn defaults_to_standard() {
        let cfg = Config::default();
        let p = resolve_parser(&cfg, None, &[]);
        assert_eq!(p.name(), "standard");
    }

    #[test]
    fn cli_overrides_config() {
        let cfg = Config {
            parser: Some(ParserConfig {
                name: "not-a-real-parser".into(),
            }),
            ..Config::default()
        };
        // CLI says "standard", config says bogus — must resolve via CLI, no panic.
        let p = resolve_parser(&cfg, Some("standard"), &[]);
        assert_eq!(p.name(), "standard");
    }

    #[test]
    #[should_panic(expected = "parser 'unknown' not registered")]
    fn unknown_name_panics() {
        let cfg = Config::default();
        let _ = resolve_parser(&cfg, Some("unknown"), &[]);
    }

    #[test]
    fn config_parser_name_used_when_cli_is_none() {
        let cfg = Config {
            parser: Some(ParserConfig {
                name: "standard".into(),
            }),
            ..Config::default()
        };
        let p = resolve_parser(&cfg, None, &[]);
        assert_eq!(p.name(), "standard");
    }

    #[test]
    #[should_panic(expected = "parser 'unknown' not registered")]
    fn config_parser_unknown_name_panics_when_cli_is_none() {
        let cfg = Config {
            parser: Some(ParserConfig {
                name: "unknown".into(),
            }),
            ..Config::default()
        };
        let _ = resolve_parser(&cfg, None, &[]);
    }

    #[test]
    #[should_panic(expected = "parser '' not registered")]
    fn empty_cli_parser_string_panics() {
        let cfg = Config::default();
        let _ = resolve_parser(&cfg, Some(""), &[]);
    }

    #[test]
    #[should_panic(expected = "failed to load schema parser plugin")]
    fn nonexistent_plugin_path_panics() {
        let cfg = Config::default();
        let bad = std::path::PathBuf::from("/tmp/tablec_no_such_plugin_for_test.so");
        let _ = resolve_parser(&cfg, None, std::slice::from_ref(&bad));
    }

    #[test]
    fn returned_parser_is_usable_via_trait_object() {
        let cfg = Config::default();
        let p = resolve_parser(&cfg, None, &[]);
        let sheet: Vec<Vec<String>> = vec![
            vec!["id".into(), "name".into()],
            vec!["int".into(), "string".into()],
            vec!["".into(), "".into()],
            vec!["".into(), "".into()],
            vec!["".into(), "".into()],
        ];
        let r = p
            .parse_schema("T", &sheet)
            .expect("standard parser succeeds");
        match r {
            tablec_core::core::schema::SchemaParseResult::Schema(s) => {
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name, "id");
            }
            _ => panic!("expected Schema"),
        }
    }
}
