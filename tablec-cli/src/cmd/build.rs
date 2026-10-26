use clap::Args;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tablec_core::core::config::ResolvedConfig;
use tablec_core::core::project::project::Project;
use tablec_core::core::table::constraint::ConstraintValidator;
use tablec_core::core::table::table::read_excel;
use tablec_core::export::{Format, Json, Msgpack};

/// Format a `Duration` as a human-readable ms/s string.
fn fmt_duration(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms < 10.0 {
        format!("{:.2}ms", ms)
    } else if ms < 100.0 {
        format!("{:.1}ms", ms)
    } else if ms < 10_000.0 {
        format!("{:.0}ms", ms)
    } else {
        format!("{:.1}s", ms / 1000.0)
    }
}

/// Map an export format name to its file extension.
fn ext_for(format: &str) -> &'static str {
    if format == "msgpack" {
        "msgpack"
    } else {
        "json"
    }
}

/// Validate a single table, printing any diagnostics to stderr. Returns
/// silently on success. Validation never aborts the build.
fn validate_table_silent(table: &tablec_core::core::table::table::Table) {
    if let Err(errs) = ConstraintValidator::validate_table(table) {
        let _ = crate::diag_render::render_diags(&errs, &mut io::stderr().lock());
    }
}

#[derive(Args, Debug)]
pub struct BuildCommand {
    /// Input Excel file or directory of Excel files. When omitted, the
    /// resolved config's `data.input_dir` is used (cwd by default).
    #[arg(short, long)]
    pub input: Option<String>,

    /// Output file (merge mode / single-file). In directory mode it overrides
    /// the config-derived `{output_dir}/{project.name}.{ext}`.
    #[arg(short, long)]
    pub output: Option<String>,

    /// Config file path. Overrides discovery (cwd first, then the target
    /// input directory).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Export format: json (minified) | json-pretty (indented) | msgpack. Overrides config when given.
    #[arg(long)]
    pub format: Option<String>,

    /// Include field metadata. Overrides config when given.
    #[arg(long)]
    pub include_fields: Option<bool>,

    /// Schema parser name to use. Overrides `tablec.toml [parser] name` when given.
    /// Falls back to "standard" if neither is set.
    #[arg(long)]
    pub parser: Option<String>,

    /// Additional schema parser plugin library path. May be repeated.
    #[arg(long = "plugin-path", value_name = "PATH")]
    pub plugin_paths: Vec<String>,
}

impl BuildCommand {
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("failed to read current directory: {e}"))?;
        self.run_in(&cwd)
    }

    /// [`Self::run`] with the process cwd made explicit (unit-testable).
    fn run_in(&self, cwd: &Path) -> Result<(), Box<dyn Error>> {
        // Discovery target: the -i directory itself, a file's parent, or the
        // cwd when -i is omitted.
        let target_dir: PathBuf = match self.input.as_deref() {
            Some(i) => {
                let p = Path::new(i);
                if p.is_dir() {
                    p.to_path_buf()
                } else {
                    p.parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."))
                        .to_path_buf()
                }
            }
            None => cwd.to_path_buf(),
        };

        let resolved = ResolvedConfig::resolve_in(cwd, self.config.as_deref(), &target_dir, cwd)?;

        // Input location: -i wins; else the config's resolved input_dir.
        let scan: PathBuf = match self.input.as_deref() {
            Some(i) => PathBuf::from(i),
            None => resolved.input_dir.clone(),
        };

        if scan.is_file() {
            self.run_single(&scan, &resolved)
        } else if scan.is_dir() {
            self.run_dir(&scan, &resolved)
        } else {
            Err(format!("input {:?} is neither a file nor a directory", scan).into())
        }
    }

    fn plugin_paths_for(&self, resolved: &ResolvedConfig) -> Vec<PathBuf> {
        // Config-relative plugin paths arrive already anchored (absolute);
        // CLI --plugin-path args stay cwd-relative.
        resolved
            .plugin_paths
            .iter()
            .cloned()
            .chain(self.plugin_paths.iter().map(PathBuf::from))
            .collect()
    }

    fn run_single(&self, input: &Path, resolved: &ResolvedConfig) -> Result<(), Box<dyn Error>> {
        let cfg = &resolved.config;
        let format = self
            .format
            .clone()
            .unwrap_or_else(|| cfg.export.format.clone());
        let include_fields = self.include_fields.unwrap_or(cfg.export.include_fields);

        let plugin_paths = self.plugin_paths_for(resolved);
        let _parser =
            crate::parser_resolve::resolve_parser(cfg, self.parser.as_deref(), &plugin_paths);

        let output = self
            .output
            .as_ref()
            .ok_or("No output file specified. Use -o <file> for single-file builds.")?;

        let input_str = input.to_string_lossy().into_owned();
        let format_for_ext = format.clone();
        self.build_single_file_with_ext(
            &input_str,
            output,
            &format,
            include_fields,
            ext_for(&format_for_ext),
        )
    }

    fn run_dir(&self, scan_dir: &Path, resolved: &ResolvedConfig) -> Result<(), Box<dyn Error>> {
        let cfg = &resolved.config;
        let format = self
            .format
            .clone()
            .unwrap_or_else(|| cfg.export.format.clone());
        let include_fields = self.include_fields.unwrap_or(cfg.export.include_fields);

        let include = cfg.data.include.clone().unwrap_or_default();
        let exclude = cfg.data.exclude.clone().unwrap_or_default();
        let files = tablec_core::core::config::find_excel_files(
            &scan_dir.to_string_lossy(),
            &include,
            &exclude,
        )?;

        let plugin_paths = self.plugin_paths_for(resolved);
        let _parser =
            crate::parser_resolve::resolve_parser(cfg, self.parser.as_deref(), &plugin_paths);

        if files.is_empty() {
            return Err(format!(
                "no matching spreadsheet files in {:?} (check include/exclude patterns)",
                scan_dir
            )
            .into());
        }

        let ext = ext_for(&format).to_string();

        // Output path: -o wins; else {output_dir}/{project.name}.{ext} —
        // derived from defaults too, so a zero-config build succeeds.
        let output_path: PathBuf = match self.output.as_deref() {
            Some(s) => PathBuf::from(s),
            None => resolved
                .output_dir
                .join(format!("{}.{}", cfg.project.name, ext)),
        };

        self.build_merged_files_with_ext(&files, &output_path, &format, include_fields, &ext)
    }

    fn build_single_file_with_ext(
        &self,
        input: &str,
        output: &str,
        format: &str,
        include_fields: bool,
        ext: &str,
    ) -> Result<(), Box<dyn Error>> {
        let _ = ext;
        let total_start = Instant::now();

        let parse_start = Instant::now();
        let tables = match read_excel(input) {
            Ok(t) => t,
            Err(errs) => {
                crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
                return Err(format!(
                    "read_excel failed: {}",
                    crate::diag_render::diag_summary(&errs)
                )
                .into());
            }
        };
        let parse_elapsed = parse_start.elapsed();
        let table_count = tables.len();

        for table in &tables {
            let v_start = Instant::now();
            validate_table_silent(table);
            let v_elapsed = v_start.elapsed();
            eprintln!(
                "{}/{}: {} rows (parse={}, validate={})",
                input,
                table.name,
                table.data.len(),
                fmt_duration(parse_elapsed),
                fmt_duration(v_elapsed),
            );
        }

        let project = Project::from_tables("unnamed".to_string(), tables);

        match format {
            "json" => {
                Json {
                    pretty: false,
                    include_fields,
                }
                .export(&project, output)?;
            }
            "json-pretty" => {
                Json {
                    pretty: true,
                    include_fields,
                }
                .export(&project, output)?;
            }
            "msgpack" => {
                Msgpack.export(&project, output)?;
            }
            _ => {
                return Err(format!(
                    "Unsupported format '{}'. Use one of: json, json-pretty, msgpack.",
                    format
                )
                .into());
            }
        }

        let total_elapsed = total_start.elapsed();
        eprintln!(
            "Total: {} tables, {}",
            table_count,
            fmt_duration(total_elapsed)
        );
        Ok(())
    }

    fn build_merged_files_with_ext(
        &self,
        files: &[PathBuf],
        output: &Path,
        format: &str,
        include_fields: bool,
        ext: &str,
    ) -> Result<(), Box<dyn Error>> {
        let _ = ext;
        let output_str = output
            .to_str()
            .ok_or_else(|| format!("output path {:?} is not valid UTF-8", output))?;
        let total_start = Instant::now();

        // Merge all tables from all files; track per-file parse time and
        // emit per-table validate line as we go (before Project::from_tables
        // collapses duplicates by name).
        let mut all_tables = Vec::new();
        let mut total_table_count = 0;
        for file_path in files {
            let parse_start = Instant::now();
            let tables = match read_excel(file_path.to_str().unwrap()) {
                Ok(t) => t,
                Err(errs) => {
                    crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
                    return Err(format!(
                        "read_excel failed: {}",
                        crate::diag_render::diag_summary(&errs)
                    )
                    .into());
                }
            };
            let parse_elapsed = parse_start.elapsed();
            let file_str = file_path.to_string_lossy();

            for table in &tables {
                let v_start = Instant::now();
                validate_table_silent(table);
                let v_elapsed = v_start.elapsed();
                eprintln!(
                    "{}/{}: {} rows (parse={}, validate={})",
                    file_str,
                    table.name,
                    table.data.len(),
                    fmt_duration(parse_elapsed),
                    fmt_duration(v_elapsed),
                );
            }

            total_table_count += tables.len();
            all_tables.extend(tables);
        }

        let project = Project::from_tables("unnamed".to_string(), all_tables);

        match format {
            "json" => {
                Json {
                    pretty: false,
                    include_fields,
                }
                .export(&project, output_str)?;
                println!("Merged {} tables into {}", project.tables.len(), output_str);
            }
            "json-pretty" => {
                Json {
                    pretty: true,
                    include_fields,
                }
                .export(&project, output_str)?;
                println!("Merged {} tables into {}", project.tables.len(), output_str);
            }
            "msgpack" => {
                Msgpack.export(&project, output_str)?;
                println!("Merged tables into {}", output_str);
            }
            _ => {
                return Err(format!(
                    "Unsupported format '{}'. Use one of: json, json-pretty, msgpack.",
                    format
                )
                .into());
            }
        }

        let total_elapsed = total_start.elapsed();
        eprintln!(
            "Total: {} tables, {}",
            total_table_count,
            fmt_duration(total_elapsed)
        );
        Ok(())
    }
}

// This function is for the python library, returning the JSON as a string.
pub fn build_to_string(
    input_path: &str,
    format: &str,
    include_fields: bool,
) -> Result<String, Box<dyn Error>> {
    let tables = match read_excel(input_path) {
        Ok(t) => t,
        Err(errs) => {
            crate::diag_render::render_diags(&errs, &mut io::stderr().lock())?;
            return Err(format!(
                "read_excel failed: {}",
                crate::diag_render::diag_summary(&errs)
            )
            .into());
        }
    };
    let project = Project::from_tables("unnamed".to_string(), tables);
    match format {
        "json" => {
            let json = Json {
                pretty: false,
                include_fields,
            };
            let bytes = json.to_vec(&project)?;
            Ok(String::from_utf8(bytes)?)
        }
        "json-pretty" => {
            let json = Json {
                pretty: true,
                include_fields,
            };
            let bytes = json.to_vec(&project)?;
            Ok(String::from_utf8(bytes)?)
        }
        // Other formats could be added here if needed.
        _ => Err(format!(
            "Unsupported format '{}'. Use 'json' or 'json-pretty'.",
            format
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_duration;
    use std::time::Duration;

    #[test]
    fn fmt_duration_sub_millisecond() {
        let s = fmt_duration(Duration::from_micros(500));
        assert!(s.ends_with("ms"), "got: {}", s);
        // 0.5ms — should be 2 decimal places
        assert!(s.starts_with("0.5"), "got: {}", s);
    }

    #[test]
    fn fmt_duration_one_digit_ms() {
        let s = fmt_duration(Duration::from_micros(2_400));
        assert_eq!(s, "2.40ms");
    }

    #[test]
    fn fmt_duration_two_digit_ms() {
        let s = fmt_duration(Duration::from_micros(45_000));
        assert_eq!(s, "45.0ms");
    }

    #[test]
    fn fmt_duration_three_digit_ms() {
        let s = fmt_duration(Duration::from_micros(250_000));
        assert_eq!(s, "250ms");
    }

    #[test]
    fn fmt_duration_seconds() {
        let s = fmt_duration(Duration::from_micros(12_500_000));
        assert_eq!(s, "12.5s");
    }

    use super::{BuildCommand, ext_for};
    use rust_xlsxwriter::Workbook;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_minimal_xlsx(dir: &std::path::Path, name: &str) -> PathBuf {
        write_minimal_xlsx_with_sheet(dir, name, "Sheet1")
    }

    fn write_minimal_xlsx_with_sheet(
        dir: &std::path::Path,
        name: &str,
        sheet_name: &str,
    ) -> PathBuf {
        let path = dir.join(name);
        let mut wb = Workbook::new();
        let sheet = wb.add_worksheet();
        sheet.set_name(sheet_name).ok();
        // Standard 5-row schema: field names, types, comments, constraints, table constraints.
        sheet.write_string(0, 0, "id").ok();
        sheet.write_string(0, 1, "name").ok();
        sheet.write_string(0, 2, "value").ok();
        sheet.write_string(1, 0, "int").ok();
        sheet.write_string(1, 1, "string").ok();
        sheet.write_string(1, 2, "int").ok();
        // Row 2: comments (empty), Row 3: constraints (empty), Row 4: table constraints (empty)
        // Row 5 (index 5): data
        sheet.write_number(5, 0, 1.0).ok();
        sheet.write_string(5, 1, "alice").ok();
        sheet.write_number(5, 2, 100.0).ok();
        wb.save(&path).unwrap();
        path
    }

    fn cmd_with(
        input: Option<String>,
        output: Option<String>,
        config: Option<PathBuf>,
    ) -> BuildCommand {
        BuildCommand {
            input,
            output,
            config,
            format: None,
            include_fields: None,
            parser: None,
            plugin_paths: Vec::new(),
        }
    }

    #[test]
    fn ext_for_msgpack() {
        assert_eq!(ext_for("msgpack"), "msgpack");
    }

    #[test]
    fn ext_for_json() {
        assert_eq!(ext_for("json"), "json");
    }

    #[test]
    fn ext_for_json_pretty() {
        assert_eq!(ext_for("json-pretty"), "json");
    }

    #[test]
    fn dir_mode_with_explicit_input_and_output() {
        let dir = tempdir().unwrap();
        write_minimal_xlsx_with_sheet(dir.path(), "a.xlsx", "SheetA");
        write_minimal_xlsx_with_sheet(dir.path(), "b.xlsx", "SheetB");
        fs::write(dir.path().join("notes.csv"), "ignore me").unwrap();

        // cwd elsewhere: the explicit -i input governs scanning regardless.
        let cwd = tempdir().unwrap();
        let cmd = cmd_with(
            Some(dir.path().to_string_lossy().into_owned()),
            Some(dir.path().join("out.json").to_string_lossy().into_owned()),
            None,
        );
        cmd.run_in(cwd.path())
            .expect("dir mode build should succeed");

        // One merged output file containing both sheets
        let out_text = fs::read_to_string(dir.path().join("out.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out_text).unwrap();
        let tables = parsed
            .get("tables")
            .and_then(|v| v.as_array())
            .expect("tables array");
        assert_eq!(tables.len(), 2, "expected 2 tables, got: {}", out_text);
        let names: Vec<&str> = tables
            .iter()
            .map(|t| t.get("name").and_then(|n| n.as_str()).unwrap_or(""))
            .collect();
        assert!(names.contains(&"SheetA") && names.contains(&"SheetB"));
    }

    #[test]
    fn dir_mode_discovers_config_in_input_dir() {
        let dir = tempdir().unwrap();
        write_minimal_xlsx_with_sheet(dir.path(), "only.xlsx", "Only");
        fs::write(
            dir.path().join("tablec.toml"),
            r#"
[project]
name = "auto"

[data]
input_dir = "."

[export]
format = "json"
output_dir = "."
"#,
        )
        .unwrap();

        // No cwd config, so the target-dir config governs (name "auto" drives
        // the derived output when -o is absent — here -o wins).
        let cwd = tempdir().unwrap();
        let cmd = cmd_with(
            Some(dir.path().to_string_lossy().into_owned()),
            Some(dir.path().join("out.json").to_string_lossy().into_owned()),
            None,
        );
        cmd.run_in(cwd.path())
            .expect("dir mode build should succeed");

        let out_text = fs::read_to_string(dir.path().join("out.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out_text).unwrap();
        assert!(
            parsed
                .get("tables")
                .and_then(|v| v.as_array())
                .unwrap()
                .len()
                >= 1
        );
    }

    #[test]
    fn dir_mode_errors_when_no_matching_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), "no xlsx here").unwrap();

        let cwd = tempdir().unwrap();
        let cmd = cmd_with(
            Some(dir.path().to_string_lossy().into_owned()),
            Some(dir.path().join("out.json").to_string_lossy().into_owned()),
            None,
        );
        let err = cmd
            .run_in(cwd.path())
            .expect_err("should fail with no xlsx files");
        assert!(
            err.to_string().contains("no matching spreadsheet files"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn single_file_still_requires_output() {
        let dir = tempdir().unwrap();
        let xlsx = write_minimal_xlsx(dir.path(), "foo.xlsx");

        let cwd = tempdir().unwrap();
        let cmd = cmd_with(Some(xlsx.to_string_lossy().into_owned()), None, None);
        let err = cmd
            .run_in(cwd.path())
            .expect_err("single file without -o should fail");
        assert!(
            err.to_string().contains("No output file specified"),
            "unexpected error: {}",
            err
        );
    }

    /// Zero-config bare build: no -i, no -o, no config anywhere — scans the
    /// cwd and derives `output/default.json`.
    #[test]
    fn bare_build_zero_config_writes_default_output() {
        let cwd = tempdir().unwrap();
        write_minimal_xlsx_with_sheet(cwd.path(), "t.xlsx", "T");

        let cmd = cmd_with(None, None, None);
        cmd.run_in(cwd.path()).expect("bare build should succeed");

        let out = cwd.path().join("output").join("default.json");
        let out_text = fs::read_to_string(&out).expect("output/default.json exists");
        let parsed: serde_json::Value = serde_json::from_str(&out_text).unwrap();
        let tables = parsed.get("tables").and_then(|v| v.as_array()).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].get("name").and_then(|n| n.as_str()), Some("T"));
    }

    /// No -i: the config's input_dir governs scanning, anchored at the
    /// config file's directory.
    #[test]
    fn input_dir_from_config_when_i_omitted() {
        let cwd = tempdir().unwrap();
        std::fs::create_dir(cwd.path().join("sheets")).unwrap();
        write_minimal_xlsx_with_sheet(&cwd.path().join("sheets"), "s.xlsx", "S");
        fs::write(
            cwd.path().join("tablec.toml"),
            "[data]\ninput_dir = \"sheets\"\n\n[export]\noutput_dir = \"staging\"",
        )
        .unwrap();

        let cmd = cmd_with(None, None, None);
        cmd.run_in(cwd.path())
            .expect("config-driven build should succeed");

        // Output derives from the config: staging/default.json (project name
        // defaults to "default").
        let out = cwd.path().join("staging").join("default.json");
        let out_text = fs::read_to_string(&out).expect("staging/default.json exists");
        let parsed: serde_json::Value = serde_json::from_str(&out_text).unwrap();
        let tables = parsed.get("tables").and_then(|v| v.as_array()).unwrap();
        assert_eq!(tables[0].get("name").and_then(|n| n.as_str()), Some("S"));
    }

    /// cwd config beats the target directory's config (cwd-first discovery).
    #[test]
    fn cwd_config_wins_over_input_dir_config() {
        let cwd = tempdir().unwrap();
        fs::write(
            cwd.path().join("tablec.toml"),
            "[project]\nname = \"cwdcfg\"\n\n[data]\ninput_dir = \".\"\n\n[export]\nformat = \"json\"\noutput_dir = \"from-cwd\"",
        )
        .unwrap();

        let target = tempdir().unwrap();
        write_minimal_xlsx_with_sheet(target.path(), "t.xlsx", "T");
        fs::write(
            target.path().join("tablec.toml"),
            "[project]\nname = \"targetcfg\"\n\n[data]\ninput_dir = \".\"\n\n[export]\nformat = \"json\"\noutput_dir = \"from-target\"",
        )
        .unwrap();

        // -i points at the target dir, but the cwd config governs export.
        let cmd = cmd_with(
            Some(target.path().to_string_lossy().into_owned()),
            None,
            None,
        );
        cmd.run_in(cwd.path()).expect("build should succeed");

        let out = cwd.path().join("from-cwd").join("cwdcfg.json");
        assert!(
            out.exists(),
            "expected cwd-config-derived output at {}",
            out.display()
        );
        assert!(
            !cwd.path().join("from-target").exists(),
            "target-dir config must not govern"
        );
    }

    /// Empty cwd with no -i and no config: the no-files error replaces the
    /// old missing--o error.
    #[test]
    fn bare_build_empty_dir_errors_with_no_matching_files() {
        let cwd = tempdir().unwrap();
        let cmd = cmd_with(None, None, None);
        let err = cmd.run_in(cwd.path()).expect_err("empty cwd should fail");
        assert!(
            err.to_string().contains("no matching spreadsheet files"),
            "unexpected error: {}",
            err
        );
    }
}
