use clap::Args;
use std::error::Error;
use std::path::{Path, PathBuf};
use tablec_core::core::check::{CheckOutcome, check_project};
use tablec_core::core::config::{Config, ResolvedConfig};
use tablec_core::core::diagnostic::{Diagnostic, Severity};
use tablec_core::export::report::Report;

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

    /// Write a validation report to PATH. May be repeated; the format is
    /// inferred from the file extension (`.json` / `.html`).
    #[arg(long = "report", value_name = "PATH")]
    pub report: Vec<PathBuf>,
}

/// A requested report format, inferred from the file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Json,
    Html,
}

impl ReportFormat {
    fn render(self, report: &Report) -> String {
        match self {
            ReportFormat::Json => report.render_json(),
            ReportFormat::Html => report.render_html(),
        }
    }
}

/// Infer a report's format from the requested path's extension: `.json`
/// produces a JSON report, `.html` an HTML report. Any other extension,
/// including a missing one, is rejected with the supported list.
/// Case-insensitive on the extension itself.
pub fn resolve_report_format(path: &Path) -> Result<ReportFormat, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json") => Ok(ReportFormat::Json),
        Some("html") => Ok(ReportFormat::Html),
        _ => Err(format!(
            "unsupported report extension for '{}' — supported extensions: .json, .html",
            path.display()
        )),
    }
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

/// Render every requested report from one [`Report`] value, overwrite each
/// path, and print the written paths to stdout. A write failure is fatal
/// with an error naming the unwritable path — even when the check itself
/// found no errors (the user asked for an artifact; silently succeeding
/// without it would be worse).
fn write_report_files(
    report: &Report,
    targets: &[(PathBuf, ReportFormat)],
) -> Result<(), Box<dyn Error>> {
    for (path, format) in targets {
        let content = format.render(report);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to write report '{}': {e}", path.display()))?;
        }
        std::fs::write(path, content)
            .map_err(|e| format!("failed to write report '{}': {e}", path.display()))?;
        println!("Report written: {}", path.display());
    }
    Ok(())
}

fn _run(c: CheckCommand) -> Result<(), Box<dyn Error>> {
    println!("Checking tables...");
    let cwd =
        std::env::current_dir().map_err(|e| format!("failed to read current directory: {e}"))?;
    _run_in(c, &cwd)
}

/// [`_run`] with the process cwd made explicit (unit-testable).
fn _run_in(c: CheckCommand, cwd: &Path) -> Result<(), Box<dyn Error>> {
    // Fail fast on every requested report extension before any file scan:
    // a bad request must produce no report files and scan nothing.
    let report_targets: Vec<(PathBuf, ReportFormat)> = c
        .report
        .iter()
        .map(|p| resolve_report_format(p).map(|f| (p.clone(), f)))
        .collect::<Result<_, _>>()?;

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
    let parser_name = parser.name().to_string();

    // CLI path takes precedence; otherwise the config's resolved input_dir.
    let path: PathBuf = match c.path {
        Some(p) => p,
        None => resolved.input_dir.clone(),
    };

    if !path.exists() {
        println!("No Excel files found to check.");
        // No outcome was produced: the report records a run that checked
        // nothing, naming the requested target as the input directory.
        let outcome = CheckOutcome {
            tables: Vec::new(),
            diagnostics: Vec::new(),
            files: Vec::new(),
        };
        let report = Report::from_outcome(
            &outcome,
            env!("CARGO_PKG_VERSION"),
            &path.to_string_lossy(),
            &parser_name,
        );
        write_report_files(&report, &report_targets)?;
        return Ok(());
    }
    let (input_dir, include, exclude) = target_scope(&path, &config);

    let outcome = check_project(
        &input_dir.to_string_lossy(),
        &include,
        &exclude,
        parser.as_ref(),
    )?;

    // One Report value per run; every requested format renders from it.
    let report = Report::from_outcome(
        &outcome,
        env!("CARGO_PKG_VERSION"),
        &input_dir.to_string_lossy(),
        &parser_name,
    );

    if is_no_files_notice(&outcome.diagnostics) {
        println!("No Excel files found to check.");
        write_report_files(&report, &report_targets)?;
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
        // The report is an artifact of this failed run: write it before the
        // exit code goes non-zero.
        write_report_files(&report, &report_targets)?;
        // Return an error to indicate failure
        return Err(format!("{} errors found during check", total_errors).into());
    }

    println!("\nCheck finished successfully. No errors found.");
    write_report_files(&report, &report_targets)?;
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
            report: Vec::new(),
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
            report: Vec::new(),
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

    // -------------------------------------------------------------------
    // 2.1 --report extension resolution
    // -------------------------------------------------------------------

    #[test]
    fn report_extensions_resolve_to_formats() {
        assert_eq!(
            resolve_report_format(Path::new("out/report.json")),
            Ok(ReportFormat::Json)
        );
        assert_eq!(
            resolve_report_format(Path::new("out/report.html")),
            Ok(ReportFormat::Html)
        );
    }

    #[test]
    fn unsupported_report_extensions_rejected_naming_supported_ones() {
        for p in ["out/report.htm", "out/report", "out/report.xml"] {
            let err = resolve_report_format(Path::new(p)).unwrap_err();
            assert!(err.contains(".json"), "{p}: error must name .json: {err}");
            assert!(err.contains(".html"), "{p}: error must name .html: {err}");
            assert!(err.contains(p), "{p}: error must name the path: {err}");
        }
    }

    #[test]
    fn report_extension_is_case_insensitive() {
        assert_eq!(
            resolve_report_format(Path::new("out/report.JSON")),
            Ok(ReportFormat::Json)
        );
        assert_eq!(
            resolve_report_format(Path::new("out/report.Html")),
            Ok(ReportFormat::Html)
        );
    }

    // -------------------------------------------------------------------
    // 2.2 Report writing on every exit path
    // -------------------------------------------------------------------

    fn cmd_with_report(path: Option<PathBuf>, reports: &[PathBuf]) -> CheckCommand {
        CheckCommand {
            config: None,
            verbose: false,
            path,
            parser: None,
            plugin_paths: Vec::new(),
            report: reports.to_vec(),
        }
    }

    /// A sheet with an out-of-range int8 value → one parse error diagnostic.
    fn write_sheet_with_error(path: &Path, name: &str) {
        let mut wb = Workbook::new();
        let sheet = wb.add_worksheet();
        sheet.set_name(name).ok();
        sheet.write_string(0, 0, "id").ok();
        sheet.write_string(1, 0, "int8").ok();
        sheet.write_string(2, 0, "").ok();
        sheet.write_string(3, 0, "").ok();
        sheet.write_string(4, 0, "").ok();
        sheet.write_string(5, 0, "200").ok();
        wb.save(path).unwrap();
    }

    #[test]
    fn clean_run_writes_report_with_zero_counts() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");
        let report_path = dir.path().join("report.json");

        let r = _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[report_path.clone()]),
            dir.path(),
        );
        r.expect("clean check must succeed");

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["summary"]["errors"], 0);
        assert_eq!(v["summary"]["warnings"], 0);
        assert_eq!(v["summary"]["tables_checked"], 1);
        assert_eq!(
            v["diagnostics"].as_array().unwrap().len(),
            0,
            "clean run has an empty diagnostics array"
        );
    }

    #[test]
    fn error_run_writes_report_and_exits_non_zero() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet_with_error(&dir.path().join("bad.xlsx"), "Bad");
        let report_path = dir.path().join("report.json");

        let r = _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[report_path.clone()]),
            dir.path(),
        );
        assert!(r.is_err(), "errors keep the exit status non-zero");

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(v["summary"]["errors"], 1);
        let diags = v["diagnostics"].as_array().unwrap();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0]["severity"], "Error");
    }

    #[test]
    fn no_matching_files_run_writes_report() {
        let dir = tempfile::tempdir().unwrap(); // exists, holds no *.xlsx
        let report_path = dir.path().join("report.json");

        let r = _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[report_path.clone()]),
            dir.path(),
        );
        r.expect("a no-files notice is not an error");

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(v["summary"]["errors"], 0);
        assert_eq!(v["summary"]["warnings"], 1, "the no-files warning counts");
        assert_eq!(v["summary"]["files_checked"], 0);
        assert_eq!(v["summary"]["tables_checked"], 0);
    }

    #[test]
    fn non_existent_target_writes_report_of_empty_run() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no_such.xlsx");
        let report_path = dir.path().join("report.json");

        let r = _run_in(
            cmd_with_report(Some(missing.clone()), &[report_path.clone()]),
            dir.path(),
        );
        r.expect("missing target stays a non-error notice");

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(v["summary"]["errors"], 0);
        assert_eq!(v["summary"]["warnings"], 0);
        assert_eq!(v["summary"]["files_checked"], 0);
        assert_eq!(v["summary"]["tables_checked"], 0);
        assert_eq!(
            v["input_dir"],
            missing.to_string_lossy().as_ref(),
            "the report names the requested (missing) target"
        );
    }

    #[test]
    fn report_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");
        let report_path = dir.path().join("report.html");
        std::fs::write(&report_path, "stale junk").unwrap();

        _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[report_path.clone()]),
            dir.path(),
        )
        .expect("clean check");

        let content = std::fs::read_to_string(&report_path).unwrap();
        assert!(
            content.starts_with("<!DOCTYPE html>"),
            "stale file must be replaced by the new run's HTML report"
        );
    }

    #[test]
    fn both_formats_written_from_one_run_with_matching_summary() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");
        let json_path = dir.path().join("report.json");
        let html_path = dir.path().join("report.html");

        _run_in(
            cmd_with_report(
                Some(dir.path().to_path_buf()),
                &[json_path.clone(), html_path.clone()],
            ),
            dir.path(),
        )
        .expect("clean check");

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
        let html = std::fs::read_to_string(&html_path).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert_eq!(
            v["summary"]["errors"].as_u64().unwrap(),
            0,
            "both artifacts describe the same clean run"
        );
        assert_eq!(v["summary"]["tables_checked"], 1);
        assert!(html.contains("<li>A</li>"), "sheet A is listed as passed");
    }

    #[test]
    fn html_extension_writes_an_html_report() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");
        let report_path = dir.path().join("report.html");

        _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[report_path.clone()]),
            dir.path(),
        )
        .expect("clean check");

        let html = std::fs::read_to_string(&report_path).unwrap();
        assert!(html.contains("校验报告"), "Chinese UI labels present");
    }

    #[test]
    fn unwritable_report_path_fails_even_on_clean_check() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");
        // A report path *under a regular file*: the check is clean but the
        // artifact cannot be written.
        let blocker = dir.path().join("blocker.json");
        std::fs::write(&blocker, "i am a file").unwrap();
        let report_path = blocker.join("report.json");

        let r = _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[report_path.clone()]),
            dir.path(),
        );
        let err = r.expect_err("write failure must fail the command");
        let msg = err.to_string();
        assert!(
            msg.contains("failed to write report"),
            "error must identify the failure: {msg}"
        );
        assert!(
            msg.contains(report_path.to_string_lossy().as_ref()),
            "error must name the unwritable path: {msg}"
        );
    }

    #[test]
    fn unsupported_extension_fails_before_scanning_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");
        let bad_report = dir.path().join("report.xml");

        let r = _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[bad_report.clone()]),
            dir.path(),
        );
        let err = r.expect_err("unsupported extension must fail");
        assert!(
            err.to_string().contains(".json") && err.to_string().contains(".html"),
            "error must name the supported extensions: {err}"
        );
        assert!(!bad_report.exists(), "no report file may be written");
    }

    // -------------------------------------------------------------------
    // 2.3 No report file without the flag
    // -------------------------------------------------------------------

    #[test]
    fn no_flag_creates_no_report_file() {
        let dir = tempfile::tempdir().unwrap();
        write_sheet(&dir.path().join("a.xlsx"), "A");

        _run_in(
            cmd_with_report(Some(dir.path().to_path_buf()), &[]),
            dir.path(),
        )
        .expect("clean check without --report");

        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json") || n.ends_with(".html"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "no report file may be created without --report, found: {leftovers:?}"
        );
    }
}
