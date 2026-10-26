use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub project: ProjectConfig,
    pub data: DataConfig,
    pub export: ExportConfig,
    pub parser: Option<ParserConfig>,
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ParserConfig {
    pub name: String,
}

impl Default for ParserConfig {
    fn default() -> Self {
        ParserConfig {
            name: "standard".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PluginConfig {
    pub path: String,
}

impl Default for PluginConfig {
    fn default() -> Self {
        PluginConfig {
            path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig {
            name: "default".to_string(),
            version: None,
            description: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DataConfig {
    pub input_dir: String,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl Default for DataConfig {
    fn default() -> Self {
        DataConfig {
            input_dir: ".".to_string(),
            include: Some(vec!["*.xlsx".to_string()]),
            exclude: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ExportConfig {
    pub format: String,
    pub output_dir: String,
    pub include_fields: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        ExportConfig {
            format: "json".to_string(),
            output_dir: "output".to_string(),
            include_fields: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            project: ProjectConfig::default(),
            data: DataConfig::default(),
            export: ExportConfig::default(),
            parser: None,
            plugins: Vec::new(),
        }
    }
}

impl Config {
    /// Load a config from an explicit file path. Errors name the file so a
    /// typo'd key is reported against the config it lives in.
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        let content = std::fs::read_to_string(path).map_err(|e| -> Box<dyn Error> {
            format!("failed to read config {}: {e}", path.display()).into()
        })?;
        toml::from_str(&content).map_err(|e| -> Box<dyn Error> {
            format!("invalid config {}: {e}", path.display()).into()
        })
    }
}

/// A [`Config`] together with every path it expresses, resolved to absolute
/// paths. Consumers take `input_dir` / `output_dir` / `plugin_paths` from
/// here instead of joining strings themselves.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// The config as written (raw, un-anchored values).
    pub config: Config,
    /// File the config was loaded from; `None` means all defaults.
    pub config_path: Option<PathBuf>,
    /// Directory relative paths anchor to: the config file's directory, or
    /// the default base when no config file was found.
    pub base_dir: PathBuf,
    /// `data.input_dir` anchored to `base_dir` (absolute).
    pub input_dir: PathBuf,
    /// `export.output_dir` anchored to `base_dir` (absolute).
    pub output_dir: PathBuf,
    /// Each `plugins[].path` anchored to `base_dir` (absolute).
    pub plugin_paths: Vec<PathBuf>,
}

impl ResolvedConfig {
    /// Resolve the config for a command run from the process cwd.
    ///
    /// Discovery precedence: `explicit` (`--config`) > `tablec.toml` /
    /// `.tablec.toml` in the cwd > same in `target_dir` > all defaults.
    /// Relative config paths anchor to the config file's directory; with no
    /// file found they anchor to the cwd.
    pub fn resolve(explicit: Option<&Path>, target_dir: &Path) -> Result<Self, Box<dyn Error>> {
        let cwd = std::env::current_dir().map_err(|e| -> Box<dyn Error> {
            format!("failed to read current directory: {e}").into()
        })?;
        Self::resolve_in(&cwd, explicit, target_dir, &cwd)
    }

    /// [`Self::resolve`] with the cwd and the no-file base directory made
    /// explicit. The webui passes its `--dir` as `default_base` so a config
    /// less run anchors paths to the pointed-at directory instead of the
    /// process cwd.
    pub fn resolve_in(
        cwd: &Path,
        explicit: Option<&Path>,
        target_dir: &Path,
        default_base: &Path,
    ) -> Result<Self, Box<dyn Error>> {
        let (config, config_path) = if let Some(p) = explicit {
            (Config::load_from_file(p)?, Some(p.to_path_buf()))
        } else if let Some(p) = find_config_in(cwd) {
            (Config::load_from_file(&p)?, Some(p))
        } else if let Some(p) = find_config_in(target_dir) {
            (Config::load_from_file(&p)?, Some(p))
        } else {
            (Config::default(), None)
        };

        let base_dir = config_path
            .as_ref()
            .and_then(|p| p.parent())
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| default_base.to_path_buf());

        Ok(ResolvedConfig {
            input_dir: anchor(&base_dir, &config.data.input_dir),
            output_dir: anchor(&base_dir, &config.export.output_dir),
            plugin_paths: config
                .plugins
                .iter()
                .map(|p| anchor(&base_dir, &p.path))
                .collect(),
            config,
            config_path,
            base_dir,
        })
    }
}

/// Look for `tablec.toml` (preferred) or `.tablec.toml` inside `dir`.
fn find_config_in(dir: &Path) -> Option<PathBuf> {
    for name in ["tablec.toml", ".tablec.toml"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Anchor `value` to `base`: absolute values pass through, `"."` is the base
/// itself, anything else joins onto it. No canonicalization — the path stays
/// lexical.
fn anchor(base: &Path, value: &str) -> PathBuf {
    let p = Path::new(value);
    if p.is_absolute() {
        p.to_path_buf()
    } else if value == "." {
        base.to_path_buf()
    } else {
        base.join(p)
    }
}

/// Find Excel files matching the given patterns in the input directory
pub fn find_excel_files(
    input_dir: &str,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    let dir_path = Path::new(input_dir);

    if !dir_path.exists() {
        return Ok(files);
    }

    // Default include pattern if none specified
    let include_patterns = if include.is_empty() {
        vec!["*.xlsx".to_string()]
    } else {
        include.to_vec()
    };

    for pattern in &include_patterns {
        // Handle glob patterns
        let full_pattern = if pattern.starts_with("**/") {
            format!("{}{}", input_dir, &pattern[2..])
        } else if pattern.contains('*') {
            format!("{}/{}", input_dir, pattern)
        } else {
            format!("{}/{}", input_dir, pattern)
        };

        for entry in glob::glob(&full_pattern)? {
            match entry {
                Ok(path) => {
                    // Check exclude patterns
                    let should_exclude = exclude.iter().any(|excl| {
                        let excl_pattern = if excl.starts_with("**/") {
                            format!("{}{}", input_dir, &excl[2..])
                        } else {
                            format!("{}/{}", input_dir, excl)
                        };
                        if let Ok(mut matches) = glob::glob(&excl_pattern) {
                            matches.any(|m| m.map(|p| p == path).unwrap_or(false))
                        } else {
                            false
                        }
                    });

                    if !should_exclude {
                        // Verify it's an Excel file
                        if let Some(ext) = path.extension() {
                            if ext == "xlsx" || ext == "xls" {
                                files.push(path);
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Error reading glob entry: {}", e),
            }
        }
    }

    // Sort and deduplicate
    files.sort();
    files.dedup();

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Schema: defaults, minimal files, unknown keys
    // ---------------------------------------------------------------------------

    #[test]
    fn default_config_matches_documented_values() {
        let c = Config::default();
        assert_eq!(c.project.name, "default");
        assert_eq!(c.project.version, None);
        assert_eq!(c.data.input_dir, ".");
        assert_eq!(
            c.data.include.as_deref(),
            Some(["*.xlsx".to_string()].as_slice())
        );
        assert_eq!(c.data.exclude, None);
        assert_eq!(c.export.format, "json");
        assert_eq!(c.export.output_dir, "output");
        assert!(!c.export.include_fields);
        assert!(c.parser.is_none());
        assert!(c.plugins.is_empty());
    }

    #[test]
    fn empty_toml_parses_with_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn parser_only_toml_parses() {
        let c: Config = toml::from_str("[parser]\nname = \"my-parser\"").unwrap();
        assert_eq!(c.parser.unwrap().name, "my-parser");
        // Everything else still comes from defaults.
        assert_eq!(c.data.input_dir, ".");
        assert_eq!(c.export.format, "json");
        assert_eq!(c.project.name, "default");
    }

    #[test]
    fn empty_parser_section_defaults_to_standard() {
        let c: Config = toml::from_str("[parser]").unwrap();
        assert_eq!(c.parser.unwrap().name, "standard");
    }

    #[test]
    fn plugins_array_still_parses() {
        let c: Config = toml::from_str(
            r#"
[[plugins]]
path = "./libfoo.so"

[[plugins]]
path = "./libbar.so"
"#,
        )
        .unwrap();
        assert_eq!(c.plugins.len(), 2);
        assert_eq!(c.plugins[0].path, "./libfoo.so");
        assert_eq!(c.plugins[1].path, "./libbar.so");
    }

    #[test]
    fn full_config_parses_without_pretty() {
        let c: Config = toml::from_str(
            r#"
[project]
name = "demo"
version = "1.0"
description = "d"

[data]
input_dir = "sheets"
include = ["*.xlsx"]
exclude = ["~draft*.xlsx"]

[export]
format = "json-pretty"
output_dir = "build"
include_fields = true

[parser]
name = "p"
"#,
        )
        .unwrap();
        assert_eq!(c.project.name, "demo");
        assert_eq!(c.data.input_dir, "sheets");
        assert_eq!(c.export.format, "json-pretty");
        assert_eq!(c.export.output_dir, "build");
        assert!(c.export.include_fields);
    }

    /// A typo'd key in any section must fail parsing with the key named.
    #[test]
    fn unknown_key_rejected_in_every_section() {
        let cases: &[(&str, &str)] = &[
            ("[project]\nnaem = \"x\"", "naem"),
            ("[data]\ninput_dirr = \"x\"", "input_dirr"),
            ("[export]\noutput_dirr = \"x\"", "output_dirr"),
            ("[export]\npretty = true", "pretty"),
            ("[parser]\nnmae = \"x\"", "nmae"),
            ("[[plugins]]\npat = \"x\"", "pat"),
        ];
        for (toml_str, key) in cases {
            let err = toml::from_str::<Config>(toml_str)
                .err()
                .unwrap_or_else(|| panic!("expected {key:?} to be rejected"));
            let msg = err.to_string();
            assert!(
                msg.contains("unknown field") && msg.contains(key),
                "expected error for {key:?}, got: {msg}"
            );
        }
    }

    #[test]
    fn load_from_file_names_the_config_in_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tablec.toml");
        std::fs::write(&path, "[export]\npretty = true").unwrap();
        let err = Config::load_from_file(&path).err().expect("invalid toml");
        let msg = err.to_string();
        assert!(msg.contains("invalid config"), "got: {msg}");
        assert!(msg.contains("tablec.toml"), "got: {msg}");
        assert!(msg.contains("pretty"), "got: {msg}");
    }

    #[test]
    fn load_from_file_missing_file_names_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.toml");
        let err = Config::load_from_file(&path).err().expect("missing file");
        assert!(
            err.to_string().contains("failed to read config"),
            "got: {}",
            err
        );
    }

    // ---------------------------------------------------------------------------
    // Resolution: discovery tiers and path anchoring
    // ---------------------------------------------------------------------------

    fn write_config(dir: &Path, file: &str, name: &str) -> PathBuf {
        let path = dir.join(file);
        std::fs::write(
            &path,
            format!("[project]\nname = \"{name}\"\n\n[data]\ninput_dir = \"sheets\""),
        )
        .unwrap();
        path
    }

    #[test]
    fn explicit_config_wins_over_cwd_and_target() {
        let cwd = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        write_config(cwd.path(), "tablec.toml", "cwd");
        write_config(target.path(), "tablec.toml", "target");
        let explicit = write_config(other.path(), "alt.toml", "explicit");

        let r = ResolvedConfig::resolve_in(cwd.path(), Some(&explicit), target.path(), cwd.path())
            .unwrap();
        assert_eq!(r.config.project.name, "explicit");
        assert_eq!(r.config_path.as_deref(), Some(explicit.as_path()));
    }

    #[test]
    fn cwd_config_wins_over_target_dir() {
        let cwd = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        write_config(cwd.path(), "tablec.toml", "cwd");
        write_config(target.path(), "tablec.toml", "target");

        let r = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), cwd.path()).unwrap();
        assert_eq!(r.config.project.name, "cwd");
        assert_eq!(r.base_dir, cwd.path());
    }

    #[test]
    fn target_config_used_when_cwd_has_none() {
        let cwd = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        write_config(target.path(), ".tablec.toml", "target-dot");

        let r = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), cwd.path()).unwrap();
        assert_eq!(r.config.project.name, "target-dot");
        assert_eq!(r.base_dir, target.path());
    }

    #[test]
    fn plain_tablec_toml_preferred_over_dotfile() {
        let cwd = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        write_config(target.path(), "tablec.toml", "plain");
        write_config(target.path(), ".tablec.toml", "dot");

        let r = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), cwd.path()).unwrap();
        assert_eq!(r.config.project.name, "plain");
    }

    #[test]
    fn defaults_when_no_config_anywhere() {
        let cwd = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();

        let r = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), cwd.path()).unwrap();
        assert!(r.config_path.is_none());
        assert_eq!(r.base_dir, cwd.path());
        assert_eq!(r.config, Config::default());
    }

    #[test]
    fn relative_paths_anchor_to_config_dir() {
        let target = tempfile::tempdir().unwrap();
        let path = write_config(target.path(), "tablec.toml", "anchored");
        // A genuinely absolute path passes through unchanged (host-absolute,
        // since `/x` alone is not absolute on Windows).
        let abs = std::env::temp_dir().join("tablec-abs-test/libbar.so");
        std::fs::write(
            &path,
            format!(
                "[data]\ninput_dir = \"sheets\"\n\n[export]\noutput_dir = \"out\"\n\n[[plugins]]\npath = \"plugins/libfoo.so\"\n\n[[plugins]]\npath = \"{}\"",
                abs.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();

        let cwd = tempfile::tempdir().unwrap();
        let r = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), cwd.path()).unwrap();
        assert_eq!(r.base_dir, target.path());
        assert_eq!(r.input_dir, target.path().join("sheets"));
        assert_eq!(r.output_dir, target.path().join("out"));
        assert_eq!(r.plugin_paths[0], target.path().join("plugins/libfoo.so"));
        assert_eq!(r.plugin_paths[1], abs);
    }

    #[test]
    fn dot_values_resolve_to_base_itself() {
        let target = tempfile::tempdir().unwrap();
        std::fs::write(
            target.path().join("tablec.toml"),
            "[data]\ninput_dir = \".\"\n\n[export]\noutput_dir = \".\"",
        )
        .unwrap();

        let cwd = tempfile::tempdir().unwrap();
        let r = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), cwd.path()).unwrap();
        assert_eq!(r.input_dir, target.path());
        assert_eq!(r.output_dir, target.path());
    }

    #[test]
    fn no_config_anchors_to_default_base() {
        let cwd = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();

        let r = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), target.path()).unwrap();
        assert!(r.config_path.is_none());
        assert_eq!(r.base_dir, target.path());
        // input_dir default "." resolves to the default base itself.
        assert_eq!(r.input_dir, target.path());
        assert_eq!(r.output_dir, target.path().join("output"));
    }

    #[test]
    fn explicit_config_anchors_to_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("cfg");
        std::fs::create_dir(&nested).unwrap();
        let explicit = write_config(&nested, "custom.toml", "nested");

        let cwd = tempfile::tempdir().unwrap();
        let r = ResolvedConfig::resolve_in(cwd.path(), Some(&explicit), cwd.path(), cwd.path())
            .unwrap();
        assert_eq!(r.base_dir, nested);
        assert_eq!(r.input_dir, nested.join("sheets"));
    }

    #[test]
    fn explicit_missing_config_errors() {
        let cwd = tempfile::tempdir().unwrap();
        let missing = cwd.path().join("no-such.toml");
        let err = ResolvedConfig::resolve_in(cwd.path(), Some(&missing), cwd.path(), cwd.path())
            .err()
            .expect("missing explicit config");
        assert!(
            err.to_string().contains("failed to read config"),
            "got: {}",
            err
        );
    }

    #[test]
    fn invalid_config_in_cwd_errors_loudly() {
        let cwd = tempfile::tempdir().unwrap();
        std::fs::write(cwd.path().join("tablec.toml"), "[export]\npretty = true").unwrap();
        let target = tempfile::tempdir().unwrap();

        let err = ResolvedConfig::resolve_in(cwd.path(), None, target.path(), cwd.path())
            .err()
            .expect("invalid config");
        let msg = err.to_string();
        assert!(msg.contains("pretty"), "got: {msg}");
        assert!(msg.contains("tablec.toml"), "got: {msg}");
    }

    /// The process-cwd wrapper: an explicit config resolves without touching
    /// discovery tiers.
    #[test]
    fn resolve_wrapper_loads_explicit_config() {
        let dir = tempfile::tempdir().unwrap();
        let explicit = write_config(dir.path(), "tablec.toml", "wrapper");

        let r = ResolvedConfig::resolve(Some(&explicit), dir.path()).unwrap();
        assert_eq!(r.config.project.name, "wrapper");
        assert_eq!(r.base_dir, dir.path());
    }

    // ---------------------------------------------------------------------------
    // find_excel_files: glob semantics (unchanged by this change, now tested
    // here so the module carries its own coverage)
    // ---------------------------------------------------------------------------

    mod find_tests {
        use super::*;
        use rust_xlsxwriter::Workbook;

        fn write_xlsx(dir: &Path, name: &str) {
            let mut wb = Workbook::new();
            let sheet = wb.add_worksheet();
            sheet.set_name("S").ok();
            sheet.write_string(0, 0, "id").ok();
            sheet.write_string(1, 0, "int").ok();
            wb.save(dir.join(name)).unwrap();
        }

        fn names(files: &[PathBuf]) -> Vec<String> {
            files
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        }

        #[test]
        fn nonexistent_dir_yields_empty() {
            let files = find_excel_files("Z:/no/such/dir", &["*.xlsx".to_string()], &[]).unwrap();
            assert!(files.is_empty());
        }

        #[test]
        fn empty_include_defaults_to_xlsx_glob() {
            let dir = tempfile::tempdir().unwrap();
            write_xlsx(dir.path(), "a.xlsx");
            std::fs::write(dir.path().join("b.txt"), "x").unwrap();

            let files = find_excel_files(&dir.path().to_string_lossy(), &[], &[]).unwrap();
            assert_eq!(names(&files), vec!["a.xlsx".to_string()]);
        }

        #[test]
        fn exact_name_pattern_without_glob_chars() {
            let dir = tempfile::tempdir().unwrap();
            write_xlsx(dir.path(), "a.xlsx");
            write_xlsx(dir.path(), "b.xlsx");

            let files =
                find_excel_files(&dir.path().to_string_lossy(), &["a.xlsx".to_string()], &[])
                    .unwrap();
            assert_eq!(names(&files), vec!["a.xlsx".to_string()]);
        }

        #[test]
        fn recursive_include_pattern_selects_nested_files() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir(dir.path().join("sub")).unwrap();
            write_xlsx(dir.path(), "top.xlsx");
            write_xlsx(&dir.path().join("sub"), "deep.xlsx");

            // A `**` glob COMPONENT (e.g. `/**/*.xlsx`) recurses — this is
            // the documented way to scan nested trees.
            let files = find_excel_files(
                &dir.path().to_string_lossy(),
                &["/**/*.xlsx".to_string()],
                &[],
            )
            .unwrap();
            let got = names(&files);
            assert!(got.contains(&"deep.xlsx".to_string()), "got: {got:?}");
            assert!(got.contains(&"top.xlsx".to_string()), "got: {got:?}");
        }

        /// Documented quirk: a leading `**/` prefix is stripped and the rest
        /// is joined flat, so `**/*.xlsx` matches only the top level — not a
        /// recursive pattern. Existing configs rely on the stripped form
        /// staying available, so pin the behavior.
        #[test]
        fn leading_slash_star_star_prefix_matches_top_level_only() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir(dir.path().join("sub")).unwrap();
            write_xlsx(dir.path(), "top.xlsx");
            write_xlsx(&dir.path().join("sub"), "deep.xlsx");

            let files = find_excel_files(
                &dir.path().to_string_lossy(),
                &["**/*.xlsx".to_string()],
                &[],
            )
            .unwrap();
            assert_eq!(names(&files), vec!["top.xlsx".to_string()]);
        }

        #[test]
        fn exclude_removes_matched_file() {
            let dir = tempfile::tempdir().unwrap();
            write_xlsx(dir.path(), "keep.xlsx");
            write_xlsx(dir.path(), "drop.xlsx");

            let files = find_excel_files(
                &dir.path().to_string_lossy(),
                &["*.xlsx".to_string()],
                &["drop.xlsx".to_string()],
            )
            .unwrap();
            assert_eq!(names(&files), vec!["keep.xlsx".to_string()]);
        }

        #[test]
        fn recursive_exclude_pattern_removes_nested_file() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir(dir.path().join("sub")).unwrap();
            write_xlsx(dir.path(), "keep.xlsx");
            write_xlsx(&dir.path().join("sub"), "drop.xlsx");

            let files = find_excel_files(
                &dir.path().to_string_lossy(),
                &["**/*.xlsx".to_string()],
                &["**/drop.xlsx".to_string()],
            )
            .unwrap();
            assert_eq!(names(&files), vec!["keep.xlsx".to_string()]);
        }

        #[test]
        fn non_spreadsheet_extensions_are_filtered() {
            let dir = tempfile::tempdir().unwrap();
            write_xlsx(dir.path(), "a.xlsx");
            std::fs::write(dir.path().join("b.xlsb"), "fake").unwrap();
            std::fs::write(dir.path().join("c.txt"), "x").unwrap();

            let files =
                find_excel_files(&dir.path().to_string_lossy(), &["*".to_string()], &[]).unwrap();
            assert_eq!(names(&files), vec!["a.xlsx".to_string()]);
        }

        #[test]
        fn xls_extension_is_accepted() {
            let dir = tempfile::tempdir().unwrap();
            write_xlsx(dir.path(), "legacy.xls");

            // The default include glob is `*.xlsx`, so an .xls file only
            // enters when the pattern matches it — then the extension filter
            // must accept xls as a spreadsheet.
            let files =
                find_excel_files(&dir.path().to_string_lossy(), &["*.xls".to_string()], &[])
                    .unwrap();
            assert_eq!(names(&files), vec!["legacy.xls".to_string()]);
        }

        #[test]
        fn results_are_sorted_and_deduplicated() {
            let dir = tempfile::tempdir().unwrap();
            write_xlsx(dir.path(), "b.xlsx");
            write_xlsx(dir.path(), "a.xlsx");

            let files = find_excel_files(
                &dir.path().to_string_lossy(),
                &["*.xlsx".to_string(), "*.xlsx".to_string()],
                &[],
            )
            .unwrap();
            assert_eq!(
                names(&files),
                vec!["a.xlsx".to_string(), "b.xlsx".to_string()]
            );
        }
    }
}
