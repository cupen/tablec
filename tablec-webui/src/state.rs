//! Shared state for the webui HTTP handlers.
//!
//! `WebuiState` is constructed once at startup (in [`crate::cmd::webui::WebuiCommand::run`])
//! and shared across all axum worker tasks via [`std::sync::Arc`]. It holds:
//!
//! - the directory the webui is operating on
//! - a parser registry (built from CLI-supplied plugin paths; never from HTTP input)
//! - optional config / parser overrides
//! - a small tokio mutex guarding the resolved-config cache
//!
//! Config resolution goes through the same [`ResolvedConfig`] layer as the
//! CLI (`--config` > cwd > `--dir` > defaults), with the `--dir` directory
//! standing in as the path anchor when no config file was found. The cache
//! is lazy: handlers populate it on first need, keyed by the requested dir.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tablec_core::core::config::ResolvedConfig;
use tablec_core::core::schema::SchemaParserRegistry;
use tokio::sync::Mutex;

use crate::watcher::WatcherState;

/// Cached resolution for one requested directory: the dir plus its outcome.
/// `Err` is cached too so a broken config is reported consistently instead
/// of re-failing (or re-succeeding) differently on every request.
type CachedConfig = Option<(PathBuf, Result<ResolvedConfig, String>)>;

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
    /// Lazily populated cache of the most-recently-resolved config.
    pub config_cache: Mutex<CachedConfig>,
    /// Server start time, used for the uptime string in `/api/health`.
    pub started_at: std::time::SystemTime,
    /// Live file watcher: publishes "files changed" on a broadcast channel.
    /// Mutex-wrapped so `start_watcher` can replace it after the input dir is
    /// resolved; handlers lock it only to clone the broadcast sender.
    pub watcher: std::sync::Mutex<WatcherState>,
}

impl WebuiState {
    /// Construct a fresh `WebuiState` with an unattached watcher. Call
    /// [`Self::start_watcher`] after construction to begin watching the
    /// resolved input directory.
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
            watcher: std::sync::Mutex::new(WatcherState::inactive()),
        }
    }

    /// Resolve the config for `dir` (using the process cwd as the discovery
    /// cwd). Same resolution the CLI subcommands use.
    pub async fn resolved_config_for(&self, dir: &Path) -> Result<ResolvedConfig, String> {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("failed to read current directory: {e}"))?;
        self.resolved_config_in(&cwd, dir).await
    }

    /// [`Self::resolved_config_for`] with the cwd made explicit (unit-testable).
    pub async fn resolved_config_in(
        &self,
        cwd: &Path,
        dir: &Path,
    ) -> Result<ResolvedConfig, String> {
        ResolvedConfig::resolve_in(cwd, self.config_path_override.as_deref(), dir, dir)
            .map_err(|e| e.to_string())
    }

    /// Resolve the config for `dir` and start watching its input directory.
    /// Safe to call once at startup after the state is constructed. Watcher
    /// failures never propagate — the webui keeps working with manual reload.
    pub fn start_watcher(&self, dir: &Path) {
        let input_dir = match std::env::current_dir() {
            Ok(cwd) => match ResolvedConfig::resolve_in(
                &cwd,
                self.config_path_override.as_deref(),
                dir,
                dir,
            ) {
                Ok(r) => r.input_dir,
                Err(e) => {
                    eprintln!(
                        "warning: config resolution failed ({e}); watching {} directly — \
                         use manual reload after fixing the config",
                        dir.display()
                    );
                    dir.to_path_buf()
                }
            },
            Err(_) => dir.to_path_buf(),
        };
        let watcher = WatcherState::start(&input_dir);
        *self.watcher.lock().unwrap() = watcher;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_in(dir: &Path, config_override: Option<PathBuf>) -> WebuiState {
        WebuiState::new(
            dir.to_path_buf(),
            Arc::new(SchemaParserRegistry::with_standard()),
            None,
            config_override,
        )
    }

    #[tokio::test]
    async fn default_resolution_scans_pointed_at_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state_in(tmp.path(), None);
        let r = state.resolved_config_for(tmp.path()).await.unwrap();
        assert!(r.config_path.is_none(), "expected no source path");
        assert_eq!(r.config.project.name, "default");
        assert_eq!(r.config.export.format, "json");
        // With no config file, `--dir` stands in as the path anchor: the
        // default input_dir "." resolves to the pointed-at folder itself.
        assert_eq!(r.input_dir, tmp.path());
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
include_fields = true
"#,
        )
        .unwrap();
        let state = state_in(tmp.path(), None);
        let r = state.resolved_config_for(tmp.path()).await.unwrap();
        assert_eq!(
            r.config_path.as_deref(),
            Some(tmp.path().join("tablec.toml").as_path())
        );
        assert_eq!(r.config.project.name, "fixture");
        assert_eq!(r.config.export.format, "json-pretty");
        assert!(r.config.export.include_fields);
        assert_eq!(
            r.config.data.include.as_deref(),
            Some(vec!["*.xlsx".to_string()].as_slice())
        );
        // Paths anchor at the config file's directory.
        assert_eq!(r.input_dir, tmp.path().join("data"));
        assert_eq!(r.output_dir, tmp.path().join("build"));
    }

    #[tokio::test]
    async fn prefers_tablec_toml_over_dotfile() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("tablec.toml"), PLAIN).unwrap();
        std::fs::write(tmp.path().join(".tablec.toml"), DOTFILE).unwrap();
        let state = state_in(tmp.path(), None);
        let r = state.resolved_config_for(tmp.path()).await.unwrap();
        assert_eq!(r.config.project.name, "plain");
    }

    #[tokio::test]
    async fn config_path_override_wins() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("tablec.toml"), PLAIN).unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join("alt.toml"), DOTFILE).unwrap();
        let state = state_in(tmp.path(), Some(other.path().join("alt.toml")));
        let r = state.resolved_config_for(tmp.path()).await.unwrap();
        assert_eq!(r.config.project.name, "dotfile");
        assert_eq!(
            r.config_path.as_deref(),
            Some(other.path().join("alt.toml").as_path())
        );
    }

    /// cwd-first discovery: a tablec.toml in the process cwd governs even
    /// when the `--dir` target carries its own.
    #[tokio::test]
    async fn cwd_config_wins_over_dir_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tablec.toml"), PLAIN).unwrap();
        let cwd = tempfile::tempdir().unwrap();
        std::fs::write(cwd.path().join("tablec.toml"), CWD_CFG).unwrap();

        let state = state_in(dir.path(), None);
        let r = state
            .resolved_config_in(cwd.path(), dir.path())
            .await
            .unwrap();
        assert_eq!(r.config.project.name, "cwdcfg");
        // Anchored at the cwd config's directory, not --dir.
        assert_eq!(r.input_dir, cwd.path().join("data"));
    }

    /// An invalid config resolves to `Err` naming the key — never a silent
    /// fallback to defaults.
    #[tokio::test]
    async fn invalid_config_fails_loudly() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("tablec.toml"), "[export]\npretty = true").unwrap();
        let state = state_in(tmp.path(), None);
        let err = state
            .resolved_config_for(tmp.path())
            .await
            .err()
            .expect("invalid config must fail");
        assert!(err.contains("pretty"), "got: {err}");
        assert!(err.contains("tablec.toml"), "got: {err}");
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

    const CWD_CFG: &str = r#"
[project]
name = "cwdcfg"

[data]
input_dir = "data"

[export]
format = "json"
output_dir = "out"
"#;
}
