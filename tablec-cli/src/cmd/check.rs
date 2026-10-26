use clap::Args;
use std::error::Error;
use std::path::{Path, PathBuf};
use tablec_core::core::check::check_project;
use tablec_core::core::config::{Config, ResolvedConfig};
use tablec_core::core::diagnostic::{Diagnostic, Severity};

#[derive(Args, Debug)]
pub struct CheckCommand {
    /// Config file path. Overrides discovery (cwd first, then the target directory).
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(short, long)]
    pub verbose: bool,

    #[arg()] // Positional argument
    pub path: Option<PathBuf>,

    /// Schema parser name to use. Overrides `tablec.toml [parser] name` when given.
    /// Falls back to "standard" if neither is set.
    #[arg(long)]
    pub parser: Option<String>,

    /// Additional schema parser plugin library path. May be repeated.
    #[arg(long = "plugin-path", value_name = "PATH")]
    pub plugin_paths: Vec<String>,
}

impl CheckCommand {
    pub fn run(self) -> Result<(), Box<dyn Error>> {
        return _run(self);
    }
}

/// Error-severity diagnostics drive the exit code.
fn error_count(diags: &[Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .count()
}

/// True when the outcome is the shared pipeline's "no matching files"
/// marker: a warning diagnostic and nothing of error severity.
fn is_no_files_notice(diags: &[Diagnostic]) -> bool {
    diags
        .iter()
        .any(|d| matches!(d.severity, Severity::Warning))
        && error_count(diags) == 0
}

/// Decompose the check target for the shared pipeline into
/// `(input_dir, include, exclude)`. A directory target uses the configured
/// include/exclude globs (defaulting to `*.xlsx`); an explicit file target
/// is checked exactly — its parent directory with the file name as the sole
/// include pattern, ignoring config globs.
fn target_scope(path: &Path, config: &Config) -> (PathBuf, Vec<String>, Vec<String>) {
    if path.is_dir() {
        let include = config
            .data
            .include
            .clone()
            .unwrap_or_else(|| vec!["*.xlsx".to_string()]);
        let exclude = config.data.exclude.clone().unwrap_or_default();
        (path.to_path_buf(), include, exclude)
    } else {
        // Explicit file target (path existence is checked by the caller).
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        (parent.to_path_buf(), vec![name], Vec::new())
    }
}

fn _run(c: CheckCommand) -> Result<(), Box<dyn Error>> {
    println!("Checking tables...");
    let cwd =
        std::env::current_dir().map_err(|e| format!("failed to read current directory: {e}"))?;
    _run_in(c, &cwd)
}

/// [`_run`] with the process cwd made explicit (unit-testable).
fn _run_in(c: CheckCommand, cwd: &Path) -> Result<(), Box<dyn Error>> {
    // Discovery target: the CLI path's directory (a file's parent), or the
    // cwd when no path is given.
    let target_dir: PathBuf = match &c.path {
        Some(p) if p.is_dir() => p.clone(),
        Some(p) => p
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        None => cwd.to_path_buf(),
    };

    let resolved = ResolvedConfig::resolve_in(cwd, c.config.as_deref(), &target_dir, cwd)?;
    let config = resolved.config.clone();

    // Resolve schema parser (cli > config > "standard"). The resolved parser
    // is what the shared check pipeline parses every file with. Config
    // plugin paths arrive already anchored; CLI args stay cwd-relative.
    let plugin_paths: Vec<PathBuf> = resolved
        .plugin_paths
        .iter()
        .cloned()
        .chain(c.plugin_paths.iter().map(PathBuf::from))
        .collect();
    let parser = crate::parser_resolve::resolve_parser(&config, c.parser.as_deref(), &plugin_paths);

    // CLI path takes precedence; otherwise the config's resolved input_dir.
    let path: PathBuf = match c.path {
        Some(p) => p,
        None => resolved.input_dir.clone(),
    };

    if !path.exists() {
        println!("No Excel files found to check.");
        return Ok(());
    }
    let (input_dir, include, exclude) = target_scope(&path, &config);

    let outcome = check_project(
        &input_dir.to_string_lossy(),
        &include,
        &exclude,
        parser.as_ref(),
    )?;

    if is_no_files_notice(&outcome.diagnostics) {
        println!("No Excel files found to check.");
        return Ok(());
    }

    // Per-sheet result lines for every table that parsed (sheets that failed
    // to parse are reported by the diagnostics below). Verbose mode marks
    // per-table-clean sheets with OK — the same per-table semantics the
    // command reported before project validation was added.
    for table in &outcome.tables {
        println!("  Checking sheet: {}", table.name);
        if c.verbose && table.validate_constraints().is_ok() {
            println!("    OK");
        }
    }

    // All diagnostics from the shared pipeline: parse failures, per-table
    // constraint violations, then cross-table `@ref` violations.
    crate::diag_render::render_diags(&outcome.diagnostics, &mut std::io::stderr().lock())?;

    let total_errors = error_count(&outcome.diagnostics);
    if total_errors > 0 {
        println!("\nFound {} errors.", total_errors);
        // Return an error to indicate failure
        return Err(format!("{} errors found during check", total_errors).into());
    }

    println!("\nCheck finished successfully. No errors found.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::Workbook;

    fn diag(sev: Severity) -> Diagnostic {
        Diagnostic {
            severity: sev,
            code: tablec_core::core::diagnostic::DiagnosticCode::Other,
            message: "x".into(),
            location: Default::default(),
        }
    }

    #[test]
    fn error_counts_only_error_severity() {
        assert_eq!(error_count(&[]), 0);
        assert_eq!(error_count(&[diag(Severity::Warning)]), 0);
        assert_eq!(
            error_count(&[diag(Severity::Warning), diag(Severity::Error)]),
            1
        );
        assert_eq!(
            error_count(&[diag(Severity::Error), diag(Severity::Error)]),
            2
        );
    }

    #[test]
    fn no_files_notice_requires_warning_and_no_errors() {
        assert!(is_no_files_notice(&[diag(Severity::Warning)]));
        assert!(!is_no_files_notice(&[]));
        assert!(!is_no_files_notice(&[diag(Severity::Error)]));
        assert!(!is_no_files_notice(&[
            diag(Severity::Warning),
            diag(Severity::Error)
        ]));
    }

    fn write_sheet(path: &Path, name: &str) {
        let mut wb = Workbook::new();
        let sheet = wb.add_worksheet();
        sheet.set_name(name).ok();
        sheet.write_string(0, 0, "id").ok();
        sheet.write_string(1, 0, "int").ok();
        sheet.write_string(2, 0, "").ok();
        sheet.write_string(3, 0, "").ok();
        sheet.write_string(4, 0, "").ok();
        sheet.write_string(5, 0, "1").ok();
        wb.save(path).unwrap();
    }

    #[test]
    fn dir_target_uses_configured_globs() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");
        let (input_dir, include, exclude) = target_scope(dir.path(), &Config::default());
        assert_eq!(input_dir, dir.path());
        assert_eq!(include, vec!["*.xlsx".to_string()]);
        assert!(exclude.is_empty());
    }

    #[test]
    fn file_target_is_parent_dir_plus_exact_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("b.xlsx");
        write_sheet(&file, "B");
        write_sheet(&dir.path().join("c.xlsx"), "C"); // must NOT be matched

        let (input_dir, include, exclude) = target_scope(&file, &Config::default());
        assert_eq!(input_dir, dir.path());
        assert_eq!(include, vec!["b.xlsx".to_string()]);
        assert!(exclude.is_empty());

        // The scope really selects only that one file via the shared pipeline.
        let outcome = check_project(
            &input_dir.to_string_lossy(),
            &include,
            &exclude,
            &tablec_core::core::schema::StandardSchemaParser,
        )
        .unwrap();
        assert_eq!(outcome.tables.len(), 1);
        assert_eq!(outcome.tables[0].name, "B");
    }

    #[test]
    fn file_target_ignores_config_globs() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("only_this.xlsx");
        write_sheet(&file, "Only");

        let mut cfg = Config::default();
        cfg.data.include = Some(vec!["never*.xlsx".to_string()]);
        cfg.data.exclude = Some(vec!["only_this.xlsx".to_string()]);

        let (input_dir, include, _) = target_scope(&file, &cfg);
        assert_eq!(include, vec!["only_this.xlsx".to_string()]);
        let outcome = check_project(
            &input_dir.to_string_lossy(),
            &include,
            &[],
            &tablec_core::core::schema::StandardSchemaParser,
        )
        .unwrap();
        assert_eq!(outcome.tables.len(), 1, "explicit file must be checked");
    }

    /// cwd-first discovery: a tablec.toml in the cwd governs the check even
    /// when the target directory carries its own config.
    #[test]
    fn cwd_config_wins_over_target_dir_config() {
        let cwd = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        write_sheet(&target.path().join("a.xlsx"), "A");
        write_sheet(&target.path().join("b.xlsx"), "B");

        // cwd config narrows the scan to a*.xlsx; the target dir's config
        // (default globs) would check both sheets.
        std::fs::write(
            cwd.path().join("tablec.toml"),
            "[data]\ninclude = [\"a*.xlsx\"]",
        )
        .unwrap();
        std::fs::write(
            target.path().join("tablec.toml"),
            "[data]\ninput_dir = \".\"",
        )
        .unwrap();

        let cmd = CheckCommand {
            config: None,
            verbose: false,
            path: Some(target.path().to_path_buf()),
            parser: None,
            plugin_paths: Vec::new(),
        };
        // Only sheet A is in scope when the cwd config governs — proven by
        // running the shared pipeline with the resolved scope ourselves.
        let resolved = ResolvedConfig::resolve_in(
            cwd.path(),
            cmd.config.as_deref(),
            target.path(),
            cwd.path(),
        )
        .unwrap();
        assert_eq!(
            resolved.config_path.as_deref(),
            Some(cwd.path().join("tablec.toml").as_path())
        );
        let (input_dir, include, exclude) = target_scope(target.path(), &resolved.config);
        assert_eq!(input_dir, target.path());
        assert_eq!(include, vec!["a*.xlsx".to_string()]);
        let outcome = check_project(
            &input_dir.to_string_lossy(),
            &include,
            &exclude,
            &tablec_core::core::schema::StandardSchemaParser,
        )
        .unwrap();
        assert_eq!(outcome.tables.len(), 1, "cwd config's include must govern");
        assert_eq!(outcome.tables[0].name, "A");
    }

    /// No path arg: the check target comes from the resolved config's
    /// input_dir (anchored at the config file's directory).
    #[test]
    fn no_path_uses_config_input_dir_anchored_at_config_dir() {
        let cwd = tempfile::tempdir().unwrap();
        std::fs::create_dir(cwd.path().join("sheets")).unwrap();
        write_sheet(&cwd.path().join("sheets").join("s.xlsx"), "S");
        std::fs::write(
            cwd.path().join("tablec.toml"),
            "[data]\ninput_dir = \"sheets\"",
        )
        .unwrap();

        let cmd = CheckCommand {
            config: None,
            verbose: false,
            path: None,
            parser: None,
            plugin_paths: Vec::new(),
        };
        // The command's own resolution path: no path arg → the config's
        // input_dir governs and the run must succeed end to end.
        _run_in(cmd, cwd.path()).expect("check via config input_dir should succeed");
        let resolved =
            ResolvedConfig::resolve_in(cwd.path(), None, cwd.path(), cwd.path()).unwrap();
        assert_eq!(resolved.input_dir, cwd.path().join("sheets"));
        assert!(resolved.input_dir.is_dir(), "config input_dir must resolve");
        let (input_dir, include, _) = target_scope(&resolved.input_dir, &resolved.config);
        let outcome = check_project(
            &input_dir.to_string_lossy(),
            &include,
            &[],
            &tablec_core::core::schema::StandardSchemaParser,
        )
        .unwrap();
        assert_eq!(outcome.tables.len(), 1);
        assert_eq!(outcome.tables[0].name, "S");
    }
}
