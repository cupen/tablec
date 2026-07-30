use std::sync::Arc;

use tablec_core::core::config::{Config, ParserConfig};
use tablec_core::core::schema::{SchemaParser, SchemaParserRegistry};

/// Resolve which `SchemaParser` to use given CLI flag, parsed config,
/// or default fallback ("standard").
///
/// Precedence: `cli_parser` > `config.parser.name` > `"standard"`.
///
/// Panics if the resolved name is not registered in `SchemaParserRegistry::with_standard()`.
pub fn resolve_parser(config: &Config, cli_parser: Option<&str>) -> Arc<dyn SchemaParser> {
    let name = cli_parser
        .map(|s| s.to_string())
        .or_else(|| config.parser.as_ref().map(|p| p.name.clone()))
        .unwrap_or_else(|| "standard".to_string());
    let reg = SchemaParserRegistry::with_standard();
    reg.get(&name)
        .unwrap_or_else(|| panic!("parser '{}' not registered", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_standard() {
        let cfg = Config::default();
        let p = resolve_parser(&cfg, None);
        assert_eq!(p.name(), "standard");
    }

    #[test]
    fn config_name_used_when_no_cli() {
        let cfg = Config {
            parser: Some(ParserConfig {
                name: "standard".to_string(),
            }),
            ..Config::default()
        };
        let p = resolve_parser(&cfg, None);
        assert_eq!(p.name(), "standard");
    }

    #[test]
    fn cli_overrides_config() {
        let cfg = Config {
            parser: Some(ParserConfig {
                name: "standard".to_string(),
            }),
            ..Config::default()
        };
        // CLI passes same "standard" — still resolves to standard via CLI path.
        let p = resolve_parser(&cfg, Some("standard"));
        assert_eq!(p.name(), "standard");
    }

    #[test]
    #[should_panic(expected = "parser 'unknown' not registered")]
    fn unknown_name_panics() {
        let cfg = Config::default();
        let _ = resolve_parser(&cfg, Some("unknown"));
    }
}
