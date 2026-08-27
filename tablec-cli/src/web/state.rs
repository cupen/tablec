//! Shared state for the webui HTTP handlers.
//!
//! `WebuiState` is constructed once at startup (in [`crate::cmd::webui::WebuiCommand::run`])
//! and shared across all axum worker tasks via [`std::sync::Arc`]. It holds:
//!
//! - the directory the webui is operating on
//! - a parser registry (built from CLI-supplied plugin paths; never from HTTP input)
//! - optional config / parser overrides
//! - a small tokio mutex guarding the loaded [`Config`] cache
//!
//! The config cache is lazy: handlers populate it on first need and refresh
//! it when the user clicks "reload" (`POST /api/state {reload: true}` —
//! currently the handler is GET-only; reload is exposed by re-fetching `/api/state`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tablec_core::core::config::Config;
use tablec_core::core::schema::SchemaParserRegistry;
use tokio::sync::Mutex;

/// Shared state for the webui.
pub struct WebuiState {
    /// Default directory for `dir=...` parameters that omit the query arg.
    pub dir: PathBuf,
    /// Schema parser registry built at startup.
    pub registry: Arc<SchemaParserRegistry>,
    /// Optional `--parser` override (CLI flag).
    pub parser_override: Option<String>,
    /// Optional explicit `--config` path (CLI flag).
    pub config_path_override: Option<PathBuf>,
    /// Lazily populated cache of the most-recently-loaded config.
    pub config_cache: Mutex<Option<Config>>,
    /// Server start time, used for the uptime string in `/api/health`.
    pub started_at: std::time::SystemTime,
}

impl WebuiState {
    /// Construct a fresh `WebuiState`. Call once at startup.
    pub fn new(
        dir: PathBuf,
        registry: Arc<SchemaParserRegistry>,
        parser_override: Option<String>,
        config_path_override: Option<PathBuf>,
    ) -> Self {
        Self {
            dir,
            registry,
            parser_override,
            config_path_override,
            config_cache: Mutex::new(None),
            started_at: std::time::SystemTime::now(),
        }
    }

    /// Load a `Config` for `dir`. Honors (in order):
    ///
    /// 1. `config_path_override` if set on the state
    /// 2. `<dir>/tablec.toml`
    /// 3. `<dir>/.tablec.toml`
    /// 4. [`Config::default`]
    ///
    /// Returns the resolved config together with the path it was loaded
    /// from (`None` means "default constructed, no file").
    pub async fn load_config_for(&self, dir: &Path) -> (Config, Option<PathBuf>) {
        if let Some(p) = &self.config_path_override {
            if let Ok(c) = Config::load_from_file(p) {
                return (c, Some(p.clone()));
            }
        }
        for name in ["tablec.toml", ".tablec.toml"] {
            let p = dir.join(name);
            if p.exists() {
                if let Ok(c) = Config::load_from_file(&p) {
                    return (c, Some(p));
                }
            }
        }
        (Config::default(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_config_when_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let state = WebuiState::new(
            tmp.path().to_path_buf(),
            Arc::new(SchemaParserRegistry::with_standard()),
            None,
            None,
        );
        let (cfg, from) = state.load_config_for(tmp.path()).await;
        assert!(from.is_none(), "expected no source path, got {from:?}");
        assert_eq!(cfg.project.name, "default");
        assert_eq!(cfg.export.format, "json");
    }

    #[tokio::test]
    async fn loads_tablec_toml_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("tablec.toml"),
            r#"
[project]
name = "fixture"

[data]
input_dir = "data"
include = ["*.xlsx"]

[export]
format = "json-pretty"
output_dir = "build"
pretty = true
include_fields = true
"#,
        )
        .unwrap();
        let state = WebuiState::new(
            tmp.path().to_path_buf(),
            Arc::new(SchemaParserRegistry::with_standard()),
            None,
            None,
        );
        let (cfg, from) = state.load_config_for(tmp.path()).await;
        assert_eq!(
            from.as_deref(),
            Some(tmp.path().join("tablec.toml").as_path())
        );
        assert_eq!(cfg.project.name, "fixture");
        assert_eq!(cfg.export.format, "json-pretty");
        assert_eq!(cfg.export.pretty, Some(true));
        assert_eq!(cfg.export.include_fields, Some(true));
        assert_eq!(
            cfg.data.include.as_deref(),
            Some(vec!["*.xlsx".to_string()].as_slice())
        );
    }

    #[tokio::test]
    async fn prefers_tablec_toml_over_dotfile() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("tablec.toml"), PLAIN).unwrap();
        std::fs::write(tmp.path().join(".tablec.toml"), DOTFILE).unwrap();
        let state = WebuiState::new(
            tmp.path().to_path_buf(),
            Arc::new(SchemaParserRegistry::with_standard()),
            None,
            None,
        );
        let (cfg, _) = state.load_config_for(tmp.path()).await;
        assert_eq!(cfg.project.name, "plain");
    }

    #[tokio::test]
    async fn config_path_override_wins() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("tablec.toml"), PLAIN).unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join("alt.toml"), DOTFILE).unwrap();
        let state = WebuiState::new(
            tmp.path().to_path_buf(),
            Arc::new(SchemaParserRegistry::with_standard()),
            None,
            Some(other.path().join("alt.toml")),
        );
        let (cfg, from) = state.load_config_for(tmp.path()).await;
        assert_eq!(cfg.project.name, "dotfile");
        assert_eq!(from, Some(other.path().join("alt.toml")));
    }

    const PLAIN: &str = r#"
[project]
name = "plain"

[data]
input_dir = "data"

[export]
format = "json"
output_dir = "out"
"#;

    const DOTFILE: &str = r#"
[project]
name = "dotfile"

[data]
input_dir = "data"

[export]
format = "json"
output_dir = "out"
"#;
}
